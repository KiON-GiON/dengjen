use crate::synth_config::PiperSynthesisConfig;
use dengjen_tts_core::{DengjenError, DengjenResult};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub quality: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ESpeakConfig {
    pub(crate) voice: String,
}

#[derive(Deserialize, Default, Clone)]
pub struct InferenceConfig {
    pub(crate) noise_scale: f32,
    pub(crate) length_scale: f32,
    pub(crate) noise_w: f32,
}

#[derive(Clone, Deserialize, Default)]
pub struct Language {
    pub(crate) code: String,
    #[allow(dead_code)]
    family: Option<String>,
    #[allow(dead_code)]
    region: Option<String>,
    #[allow(dead_code)]
    name_native: Option<String>,
    #[allow(dead_code)]
    name_english: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum PhonemeType {
    #[default]
    Espeak,
    Text,
    Pinyin,
    Hebrew,
    Kurmanji,
    Japanese,
}

#[derive(Deserialize, Default)]
pub struct ModelConfig {
    pub key: Option<String>,
    pub language: Option<Language>,
    pub audio: AudioConfig,
    #[serde(default)]
    pub num_speakers: u32,
    #[serde(default)]
    pub speaker_id_map: HashMap<String, i64>,
    pub(crate) streaming: Option<bool>,
    #[serde(default)]
    pub(crate) espeak: ESpeakConfig,
    pub(crate) inference: InferenceConfig,
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) num_symbols: u32,
    #[allow(dead_code)]
    #[serde(default)]
    pub(crate) phoneme_map: HashMap<i64, String>,
    pub(crate) phoneme_id_map: HashMap<String, Vec<i64>>,
    pub(crate) phoneme_type: Option<PhonemeType>,
    #[serde(default)]
    pub(crate) vowel_clusters: Option<Vec<Vec<String>>>,
    pub(crate) default_speaker_id: Option<i64>,
    pub(crate) hop_length: Option<usize>,
    #[cfg_attr(not(feature = "hebrew"), allow(dead_code))]
    pub(crate) hebrew_model_path: Option<PathBuf>,
    #[cfg_attr(not(feature = "pinyin"), allow(dead_code))]
    pub(crate) pinyin_model_dir: Option<PathBuf>,
    #[serde(default)]
    pub(crate) kurmanji_use_espeak: bool,
}

impl ModelConfig {
    /// Phoneme type to dispatch on. `ku` espeak voices resolve to `Kurmanji`
    /// when the `kurmanji` feature is on, unless `kurmanji_use_espeak` is set.
    pub(crate) fn effective_phoneme_type(&self) -> PhonemeType {
        match self.phoneme_type.unwrap_or_default() {
            PhonemeType::Espeak if self.routes_to_kurmanji() => PhonemeType::Kurmanji,
            other => other,
        }
    }

    fn routes_to_kurmanji(&self) -> bool {
        cfg!(feature = "kurmanji") && self.espeak.voice == "ku" && !self.kurmanji_use_espeak
    }
}

pub(crate) fn load_model_config(
    config_path: &Path,
) -> DengjenResult<(ModelConfig, PiperSynthesisConfig)> {
    let file = File::open(config_path).map_err(|why| {
        DengjenError::FailedToLoadResource(format!(
            "Failed to load model config: `{}`. Caused by: `{}`",
            config_path.display(),
            why
        ))
    })?;
    let model_config: ModelConfig = serde_json::from_reader(file).map_err(|why| {
        DengjenError::FailedToLoadResource(format!(
            "Failed to parse model config from file: `{}`. Caused by: `{}`",
            config_path.display(),
            why
        ))
    })?;
    let synth_config = PiperSynthesisConfig {
        speaker: None,
        noise_scale: model_config.inference.noise_scale,
        length_scale: model_config.inference.length_scale,
        noise_w: model_config.inference.noise_w,
    };
    Ok((model_config, synth_config))
}

pub fn map_phonemes_to_ids(
    phoneme_id_map: &HashMap<String, Vec<i64>>,
    phonemes: &str,
    pad_id: i64,
    bos_id: i64,
    eos_id: i64,
) -> Vec<i64> {
    let mut longest_entry_by_first_char: HashMap<char, usize> = HashMap::new();
    for entry in phoneme_id_map.keys() {
        let mut entry_chars = entry.chars();
        if let Some(first) = entry_chars.next() {
            let len = 1 + entry_chars.count();
            let slot = longest_entry_by_first_char.entry(first).or_insert(0);
            *slot = (*slot).max(len);
        }
    }
    let chars: Vec<char> = phonemes.chars().collect();

    let mut phoneme_ids = Vec::with_capacity((chars.len() + 2) * 2);
    phoneme_ids.push(bos_id);
    phoneme_ids.push(pad_id);
    let mut cursor = 0;
    while let Some(current) = chars.get(cursor) {
        let longest_entry = longest_entry_by_first_char
            .get(current)
            .copied()
            .unwrap_or(0);
        let widest = longest_entry.min(chars.len() - cursor);

        let matched = (1..=widest).rev().find_map(|width| {
            let candidate: String = chars.iter().skip(cursor).take(width).collect();
            phoneme_id_map
                .get(&candidate)
                .and_then(|entry| entry.first().map(|&id| (width, id)))
        });
        match matched {
            Some((width, id)) => {
                phoneme_ids.push(id);
                phoneme_ids.push(pad_id);
                cursor += width;
            }
            None => cursor += 1,
        }
    }
    phoneme_ids.push(eos_id);
    phoneme_ids
}

pub(crate) fn map_espeak_phonemes_to_ids(
    phoneme_id_map: &HashMap<String, Vec<i64>>,
    vowel_clusters: Option<&[Vec<String>]>,
    phonemes: &str,
    pad_id: i64,
    bos_id: i64,
    eos_id: i64,
) -> Vec<i64> {
    let mut allowed_clusters: HashSet<String> = HashSet::new();
    let mut longest_cluster_by_first_char: HashMap<char, usize> = HashMap::new();

    if let Some(clusters) = vowel_clusters {
        for cluster in clusters {
            if cluster.len() < 2 {
                continue;
            }
            let joined: String = cluster.concat();
            let mut joined_chars = joined.chars();
            if let Some(first) = joined_chars.next() {
                let char_len = 1 + joined_chars.count();
                if char_len >= 2 {
                    let slot = longest_cluster_by_first_char.entry(first).or_insert(0);
                    *slot = (*slot).max(char_len);
                    allowed_clusters.insert(joined);
                }
            }
        }
    }

    let chars: Vec<char> = phonemes.chars().collect();
    let mut phoneme_ids = Vec::with_capacity((chars.len() + 2) * 2);
    phoneme_ids.push(bos_id);
    phoneme_ids.push(pad_id);

    let mut cursor = 0;
    let mut char_buf = [0u8; 4];
    while let Some(&current) = chars.get(cursor) {
        let longest_cluster = longest_cluster_by_first_char
            .get(&current)
            .copied()
            .unwrap_or(0);
        let widest = longest_cluster.min(chars.len() - cursor);

        let matched_cluster = (2..=widest).rev().find_map(|width| {
            let candidate: String = chars.iter().skip(cursor).take(width).collect();
            allowed_clusters
                .contains(&candidate)
                .then_some((width, candidate))
        });

        if let Some((width, candidate)) = matched_cluster {
            if let Some(id) = phoneme_id_map
                .get(&candidate)
                .and_then(|entry| entry.first().copied())
            {
                phoneme_ids.push(id);
                phoneme_ids.push(pad_id);
            }
            cursor += width;
        } else {
            let key: &str = current.encode_utf8(&mut char_buf);
            if let Some(id) = phoneme_id_map
                .get(key)
                .and_then(|entry| entry.first().copied())
            {
                phoneme_ids.push(id);
                phoneme_ids.push(pad_id);
            }
            cursor += 1;
        }
    }

    phoneme_ids.push(eos_id);
    phoneme_ids
}

pub(crate) fn resolve_default_speaker_id(config: &ModelConfig) -> i64 {
    config.default_speaker_id.unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE_CONFIG_JSON: &str = r#"{
        "key": null,
        "language": null,
        "audio": {"sample_rate": 22050, "quality": null},
        "num_speakers": 1,
        "speaker_id_map": {},
        "streaming": false,
        "espeak": {"voice": "en-us"},
        "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
        "num_symbols": 256,
        "phoneme_map": {},
        "phoneme_id_map": {"^": [1], "$": [2], "_": [3], "a": [4], "aɪ": [5]}
    }"#;

    #[test]
    fn model_config_parses_multi_char_phoneme_cluster_keys() {
        let config: ModelConfig = serde_json::from_str(BASE_CONFIG_JSON).unwrap();
        assert_eq!(config.phoneme_id_map.get("aɪ"), Some(&vec![5]));
    }

    #[test]
    fn model_config_parses_a_minimal_manifest_without_espeak_or_symbol_count() {
        let json = r#"{
            "audio": {"sample_rate": 22050, "quality": null},
            "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
            "phoneme_id_map": {"^": [1], "$": [2], "_": [3], "a": [4]},
            "phoneme_type": "text"
        }"#;
        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.phoneme_type, Some(PhonemeType::Text));
        assert_eq!(config.num_speakers, 0);
        assert!(config.speaker_id_map.is_empty());
        assert_eq!(config.espeak.voice, "");
        assert_eq!(config.num_symbols, 0);
        assert!(config.phoneme_map.is_empty());
    }

    #[test]
    fn model_config_defaults_phoneme_type_default_speaker_id_and_hop_length_when_absent() {
        let config: ModelConfig = serde_json::from_str(BASE_CONFIG_JSON).unwrap();
        assert_eq!(config.phoneme_type, None);
        assert_eq!(config.default_speaker_id, None);
        assert_eq!(config.hop_length, None);
        assert_eq!(config.hebrew_model_path, None);
        assert_eq!(config.pinyin_model_dir, None);
    }

    #[test]
    fn model_config_parses_phoneme_type_default_speaker_id_and_hop_length_when_present() {
        let json = r#"{
            "key": null,
            "language": null,
            "audio": {"sample_rate": 22050, "quality": null},
            "num_speakers": 2,
            "speaker_id_map": {},
            "streaming": false,
            "espeak": {"voice": "en-us"},
            "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
            "num_symbols": 256,
            "phoneme_map": {},
            "phoneme_id_map": {"^": [1], "$": [2], "_": [3]},
            "phoneme_type": "hebrew",
            "default_speaker_id": 3,
            "hop_length": 512
        }"#;
        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.phoneme_type, Some(PhonemeType::Hebrew));
        assert_eq!(config.default_speaker_id, Some(3));
        assert_eq!(config.hop_length, Some(512));
    }

    fn ku_config(extra: &str) -> ModelConfig {
        let json = BASE_CONFIG_JSON.replace(r#""en-us""#, r#""ku""#).replacen(
            '{',
            &format!("{{{extra}"),
            1,
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn model_config_parses_an_explicit_kurmanji_phoneme_type() {
        let config = ku_config(r#""phoneme_type": "kurmanji","#);
        assert_eq!(config.effective_phoneme_type(), PhonemeType::Kurmanji);
    }

    #[test]
    #[cfg(feature = "kurmanji")]
    fn ku_espeak_voice_routes_to_kurmanji_when_the_feature_is_enabled() {
        assert_eq!(
            ku_config("").effective_phoneme_type(),
            PhonemeType::Kurmanji
        );
    }

    #[test]
    #[cfg(feature = "kurmanji")]
    fn kurmanji_use_espeak_keeps_the_espeak_backend() {
        let config = ku_config(r#""kurmanji_use_espeak": true,"#);
        assert_eq!(config.effective_phoneme_type(), PhonemeType::Espeak);
    }

    #[test]
    #[cfg(not(feature = "kurmanji"))]
    fn ku_espeak_voice_stays_on_espeak_without_the_feature() {
        assert_eq!(ku_config("").effective_phoneme_type(), PhonemeType::Espeak);
    }

    #[test]
    fn non_ku_espeak_voices_never_route_to_kurmanji() {
        let config: ModelConfig = serde_json::from_str(BASE_CONFIG_JSON).unwrap();
        assert_eq!(config.effective_phoneme_type(), PhonemeType::Espeak);
    }

    #[test]
    fn model_config_parses_hebrew_model_path_when_present() {
        let json = r#"{
            "key": null,
            "language": null,
            "audio": {"sample_rate": 22050, "quality": null},
            "num_speakers": 1,
            "speaker_id_map": {},
            "streaming": false,
            "espeak": {"voice": "en-us"},
            "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
            "num_symbols": 256,
            "phoneme_map": {},
            "phoneme_id_map": {"^": [1], "$": [2], "_": [3]},
            "hebrew_model_path": "/models/nakdimon.onnx"
        }"#;
        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            config.hebrew_model_path,
            Some(PathBuf::from("/models/nakdimon.onnx"))
        );
    }

    #[test]
    fn model_config_parses_pinyin_model_dir_when_present() {
        let json = r#"{
            "key": null,
            "language": null,
            "audio": {"sample_rate": 22050, "quality": null},
            "num_speakers": 1,
            "speaker_id_map": {},
            "streaming": false,
            "espeak": {"voice": "en-us"},
            "inference": {"noise_scale": 0.667, "length_scale": 1.0, "noise_w": 0.8},
            "num_symbols": 256,
            "phoneme_map": {},
            "phoneme_id_map": {"^": [1], "$": [2], "_": [3]},
            "pinyin_model_dir": "/models/g2pw"
        }"#;
        let config: ModelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.pinyin_model_dir, Some(PathBuf::from("/models/g2pw")));
    }

    fn single_char_phoneme_map() -> HashMap<String, Vec<i64>> {
        HashMap::from([
            ("^".to_string(), vec![1]),
            ("$".to_string(), vec![2]),
            ("_".to_string(), vec![3]),
            ("a".to_string(), vec![4]),
            ("b".to_string(), vec![5]),
        ])
    }

    #[test]
    fn map_phonemes_to_ids_wraps_with_bos_pad_and_eos() {
        let map = single_char_phoneme_map();
        let ids = map_phonemes_to_ids(&map, "ab", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 4, 3, 5, 3, 2]);
    }

    #[test]
    fn map_phonemes_to_ids_skips_unknown_chars() {
        let map = single_char_phoneme_map();
        let ids = map_phonemes_to_ids(&map, "azb", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 4, 3, 5, 3, 2]);
    }

    #[test]
    fn map_phonemes_to_ids_skips_an_entry_with_an_empty_id_list_instead_of_panicking() {
        let mut map = single_char_phoneme_map();
        map.insert("z".to_string(), vec![]);
        let ids = map_phonemes_to_ids(&map, "azb", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 4, 3, 5, 3, 2]);
    }

    #[test]
    fn map_phonemes_to_ids_greedily_matches_multi_char_cluster_over_single_chars() {
        let mut map = single_char_phoneme_map();
        map.insert("aɪ".to_string(), vec![161]);
        map.insert("ɪ".to_string(), vec![99]);
        let ids = map_phonemes_to_ids(&map, "aɪ", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 161, 3, 2]);
    }

    #[test]
    fn map_phonemes_to_ids_matches_correctly_when_key_lengths_vary_by_starting_character() {
        let map = HashMap::from([
            ("aaaaaaaaaa".to_string(), vec![1]),
            ("b".to_string(), vec![2]),
            ("ba".to_string(), vec![3]),
        ]);
        let ids = map_phonemes_to_ids(&map, "ba", 0, 100, 200);

        assert_eq!(ids, vec![100, 0, 3, 0, 200]);
    }

    #[test]
    fn resolve_default_speaker_id_uses_configured_value() {
        let config = ModelConfig {
            default_speaker_id: Some(7),
            ..Default::default()
        };
        assert_eq!(resolve_default_speaker_id(&config), 7);
    }

    #[test]
    fn resolve_default_speaker_id_falls_back_to_zero_when_unset() {
        let config = ModelConfig {
            default_speaker_id: None,
            ..Default::default()
        };
        assert_eq!(resolve_default_speaker_id(&config), 0);
    }

    #[test]
    fn load_model_config_errors_when_file_does_not_exist() {
        let path = std::path::Path::new("/nonexistent-piper-config-xyz.json");
        let result = load_model_config(path);
        assert!(matches!(result, Err(DengjenError::FailedToLoadResource(_))));
    }

    #[test]
    fn load_model_config_errors_on_malformed_json() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "dengjen-piper-test-malformed-{}.json",
            std::process::id()
        ));
        std::fs::write(&path, b"{ not valid json").unwrap();
        let result = load_model_config(&path);
        std::fs::remove_file(&path).ok();
        assert!(matches!(result, Err(DengjenError::FailedToLoadResource(_))));
    }

#[test]
    fn model_config_parses_vowel_clusters_when_present_and_defaults_to_none() {
        let base: ModelConfig = serde_json::from_str(BASE_CONFIG_JSON).unwrap();
        assert_eq!(base.vowel_clusters, None);

        let with_clusters = BASE_CONFIG_JSON.replacen(
            '{',
            r#"{"vowel_clusters": [["a", "ɪ"], ["o", "ʊ"]],"#,
            1,
        );
        let parsed: ModelConfig = serde_json::from_str(&with_clusters).unwrap();
        assert_eq!(
            parsed.vowel_clusters,
            Some(vec![
                vec!["a".to_string(), "ɪ".to_string()],
                vec!["o".to_string(), "ʊ".to_string()],
            ])
        );
    }

    #[test]
    fn map_espeak_phonemes_to_ids_merges_only_configured_vowel_clusters() {
        let mut map = single_char_phoneme_map();
        map.insert("e".to_string(), vec![18]);
        map.insert("ɪ".to_string(), vec![74]);
        map.insert("aɪ".to_string(), vec![161]);
        map.insert("eɪ".to_string(), vec![164]);

        let clusters = vec![vec!["a".to_string(), "ɪ".to_string()]];
        let ids = map_espeak_phonemes_to_ids(&map, Some(&clusters), "aɪeɪ", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 161, 3, 18, 3, 74, 3, 2]);
    }

    #[test]
    fn map_espeak_phonemes_to_ids_does_not_merge_diphthongs_without_vowel_clusters() {
        let mut map = single_char_phoneme_map();
        map.insert("ɪ".to_string(), vec![74]);
        map.insert("aɪ".to_string(), vec![161]);

        let ids_none = map_espeak_phonemes_to_ids(&map, None, "aɪ", 3, 1, 2);
        assert_eq!(ids_none, vec![1, 3, 4, 3, 74, 3, 2]);

        let ids_empty = map_espeak_phonemes_to_ids(&map, Some(&[]), "aɪ", 3, 1, 2);
        assert_eq!(ids_empty, vec![1, 3, 4, 3, 74, 3, 2]);
    }

    #[test]
    fn map_espeak_phonemes_to_ids_prefers_longest_cluster_and_skips_unmapped_clusters() {
        let mut map = single_char_phoneme_map();
        map.insert("ɪ".to_string(), vec![74]);
        map.insert("ə".to_string(), vec![59]);
        map.insert("aɪ".to_string(), vec![161]);
        map.insert("aɪə".to_string(), vec![170]);

        let clusters = vec![
            vec!["a".to_string()],
            vec!["a".to_string(), "ɪ".to_string()],
            vec!["a".to_string(), "ɪ".to_string(), "ə".to_string()],
            vec!["a".to_string(), "b".to_string()],
        ];
        let ids = map_espeak_phonemes_to_ids(&map, Some(&clusters), "aɪəaɪab", 3, 1, 2);
        assert_eq!(ids, vec![1, 3, 170, 3, 161, 3, 2]);
    }

    #[test]
    fn model_config_parses_japanese_phoneme_type() {
        let json = BASE_CONFIG_JSON.replacen('{', r#"{"phoneme_type": "japanese","#, 1);
        let config: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config.effective_phoneme_type(), PhonemeType::Japanese);
    }
}
