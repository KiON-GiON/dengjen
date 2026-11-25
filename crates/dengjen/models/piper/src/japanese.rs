use dengjen_tts_core::{DengjenError, DengjenResult};
use jpreprocess::{JPreprocess, SystemDictionaryConfig};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use crate::japanese_katakana::foreign_word_to_katakana;
use std::borrow::Cow;

pub(crate) const JAPANESE_DICT_ENV: &str = "DENGJEN_JA_DICT_DIR";
const NO_FEATURE: i32 = -50;

pub(crate) struct JapaneseG2p {
    // Lindera's boxed filters are not UnwindSafe, while extract_fullcontext only reads the engine.
    engine: AssertUnwindSafe<JPreprocess<jpreprocess::DefaultTokenizer>>,
}

fn catch_phonemization<T>(f: impl FnOnce() -> DengjenResult<T>) -> DengjenResult<T> {
    std::panic::catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(DengjenError::PhonemizationError(
            "the Japanese phonemizer panicked on this text".to_string(),
        ))
    })
}

fn unset_dictionary_message() -> String {
    format!(
        "Japanese voices need a jpreprocess dictionary: set {JAPANESE_DICT_ENV} to its directory"
    )
}

fn dictionary_error(path: &Path, cause: impl std::fmt::Display) -> DengjenError {
    DengjenError::FailedToLoadResource(format!(
        "Failed to load the Japanese dictionary at `{}` (set by {JAPANESE_DICT_ENV}): {cause}",
        path.display()
    ))
}

fn load_engine(dir: &Path) -> Result<JPreprocess<jpreprocess::DefaultTokenizer>, String> {
    let dictionary = SystemDictionaryConfig::File(dir.to_path_buf())
        .load()
        .map_err(|e| e.to_string())?;
    let engine = JPreprocess::with_dictionaries(dictionary, None);
    engine.text_to_njd("日本").map_err(|e| e.to_string())?;
    Ok(engine)
}

impl JapaneseG2p {
    pub(crate) fn from_env() -> DengjenResult<Self> {
        Self::from_env_value(std::env::var_os(JAPANESE_DICT_ENV))
    }

    pub(crate) fn from_env_value(value: Option<std::ffi::OsString>) -> DengjenResult<Self> {
        let dir = value
            .map(PathBuf::from)
            .ok_or_else(|| DengjenError::FailedToLoadResource(unset_dictionary_message()))?;
        let engine = std::panic::catch_unwind(|| load_engine(&dir))
            .unwrap_or_else(|_| {
                Err("the dictionary is corrupt or from another jpreprocess version".to_string())
            })
            .map_err(|cause| dictionary_error(&dir, cause))?;
        Ok(Self {
            engine: AssertUnwindSafe(engine),
        })
    }

    pub(crate) fn phonemize(&self, text: &str) -> DengjenResult<Vec<String>> {
        split_sentences(text)
            .into_iter()
            .map(|sentence| catch_phonemization(|| self.phonemize_sentence(sentence)))
            .filter(|result| result.as_ref().map_or(true, |p| !p.is_empty()))
            .collect()
    }

    fn preprocess_foreign_words<'a>(&self, sentence: &'a str) -> Cow<'a, str> {
        if !sentence.bytes().any(|b| b.is_ascii_alphabetic()) {
            return Cow::Borrowed(sentence);
        }

        let chars: Vec<(usize, char)> = sentence.char_indices().collect();
        let mut out = String::with_capacity(sentence.len());
        let mut cursor = 0;
        let mut idx = 0;

        while let Some(&(start, ch)) = chars.get(idx) {
            if !ch.is_ascii_alphabetic() {
                idx += 1;
                continue;
            }

            let mut end_idx = idx + 1;
            while let Some(&(_, next_ch)) = chars.get(end_idx) {
                if next_ch.is_ascii_alphabetic()
                    || (next_ch == '\''
                        && chars
                            .get(end_idx + 1)
                            .is_some_and(|&(_, after)| after.is_ascii_alphabetic()))
                {
                    end_idx += 1;
                } else {
                    break;
                }
            }

            let end = chars.get(end_idx).map_or(sentence.len(), |&(byte_idx, _)| byte_idx);
            if let Some(prefix) = sentence.get(cursor..start) {
                out.push_str(prefix);
            }
            if let Some(token) = sentence.get(start..end) {
                out.push_str(self.resolve_foreign_token(token).as_ref());
            }
            cursor = end;
            idx = end_idx;
        }

        if let Some(tail) = sentence.get(cursor..) {
            out.push_str(tail);
        }
        Cow::Owned(out)
    }

    fn resolve_foreign_token<'a>(&self, token: &'a str) -> Cow<'a, str> {
        if self.known_in_dictionary(token) {
            return Cow::Borrowed(token);
        }
        let is_short_all_caps =
            token.len() <= 3 && token.bytes().all(|b| b.is_ascii_uppercase());
        if !is_short_all_caps && token.bytes().any(|b| b.is_ascii_uppercase()) {
            let lower = token.to_ascii_lowercase();
            if self.known_in_dictionary(&lower) {
                return Cow::Owned(lower);
            }
        }
        let kana = foreign_word_to_katakana(token);
        if kana.is_empty() {
            Cow::Borrowed(token)
        } else {
            Cow::Owned(kana)
        }
    }

    fn known_in_dictionary(&self, word: &str) -> bool {
        let Ok(njd) = self.engine.text_to_njd(word) else {
            return false;
        };
        if njd.nodes.len() != 1 {
            return false;
        }
        njd.nodes.first().is_some_and(|node| {
            node.get_read().is_some() && node.get_pron().mora_size() > 0
        })
    }

    fn phonemize_sentence(&self, sentence: &str) -> DengjenResult<String> {
        let prepared = self.preprocess_foreign_words(sentence);
        let labels: Vec<String> = self
            .engine
            .extract_fullcontext(prepared.as_ref())
            .map_err(|e| DengjenError::PhonemizationError(e.to_string()))?
            .into_iter()
            .map(|label| label.to_string())
            .collect();
        Ok(labels_to_ipa(&labels, true))
    }
}

fn is_sentence_terminator(c: char) -> bool {
    matches!(c, '。' | '．' | '！' | '？' | '!' | '?')
}

fn is_trailing_delimiter(c: char) -> bool {
    matches!(
        c,
        '」' | '』' | '）' | '】' | '〉' | '》' | '”' | '’' | '"' | '\'' | ')' | ']'
    ) || c.is_whitespace()
}

pub(crate) fn split_sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut iter = text.char_indices().peekable();

    while let Some((_, ch)) = iter.next() {
        if !is_sentence_terminator(ch) {
            continue;
        }
        while let Some(&(_, next_ch)) = iter.peek() {
            if is_sentence_terminator(next_ch) {
                iter.next();
            } else {
                break;
            }
        }
        while let Some(&(_, next_ch)) = iter.peek() {
            if is_trailing_delimiter(next_ch) {
                iter.next();
            } else {
                break;
            }
        }
        let end = iter.peek().map_or(text.len(), |&(next_idx, _)| next_idx);
        if let Some(chunk) = text.get(start..end) {
            let trimmed = chunk.trim();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
        }
        start = end;
    }

    if let Some(tail) = text.get(start..) {
        let trimmed = tail.trim();
        if !trimmed.is_empty() {
            sentences.push(trimmed);
        }
    }

    sentences
}

struct ParsedLabel<'a> {
    phone: &'a str,
    a1: i32,
    a2: i32,
    a3: i32,
    e3: i32,
    f1: i32,
}

fn parse_field(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(NO_FEATURE)
}

fn extract_section<'a>(label: &'a str, marker: &str) -> Option<&'a str> {
    let (_, after_marker) = label.split_once(marker)?;
    Some(after_marker.split('/').next().unwrap_or(after_marker))
}

fn parse_label(label: &str) -> Option<ParsedLabel<'_>> {
    let phone_ctx = label.split('/').next()?;
    let (_, after_minus) = phone_ctx.split_once('-')?;
    let (phone, _) = after_minus.split_once('+')?;

    let (a1, a2, a3) = extract_section(label, "/A:")
        .and_then(|sec| {
            let mut parts = sec.split('+');
            let a1 = parse_field(parts.next()?);
            let a2 = parse_field(parts.next()?);
            let a3 = parse_field(parts.next()?);
            Some((a1, a2, a3))
        })
        .unwrap_or((NO_FEATURE, NO_FEATURE, NO_FEATURE));

    let e3 = extract_section(label, "/E:")
        .and_then(|sec| sec.split_once('!'))
        .and_then(|(_, after_bang)| after_bang.split('_').next())
        .map(parse_field)
        .unwrap_or(NO_FEATURE);

    let f1 = extract_section(label, "/F:")
        .and_then(|sec| sec.split('_').next())
        .map(parse_field)
        .unwrap_or(NO_FEATURE);

    Some(ParsedLabel {
        phone,
        a1,
        a2,
        a3,
        e3,
        f1,
    })
}

fn fold_devoiced_vowel(phone: &str, drop_devoiced_vowels: bool) -> &str {
    if !drop_devoiced_vowels {
        return phone;
    }
    match phone {
        "A" => "a",
        "I" => "i",
        "U" => "u",
        "E" => "e",
        "O" => "o",
        other => other,
    }
}

fn is_mora_final_phone(phone: &str) -> bool {
    matches!(
        phone,
        "a" | "i" | "u" | "e" | "o" | "A" | "I" | "U" | "E" | "O" | "N" | "cl"
    )
}

fn openjtalk_to_ipa(phone: &str) -> Option<&'static str> {
    Some(match phone {
        "a" => "a",
        "i" => "i",
        "u" => "ɯ",
        "e" => "e",
        "o" => "o",
        "k" => "k",
        "ky" => "kʲ",
        "kw" => "kʷ",
        "g" => "ɡ",
        "gy" => "ɡʲ",
        "gw" => "ɡʷ",
        "s" => "s",
        "sh" => "ɕ",
        "z" => "z",
        "j" => "dʑ",
        "t" => "t",
        "ts" => "ts",
        "ty" => "tʲ",
        "ch" => "tɕ",
        "d" => "d",
        "dy" => "dʲ",
        "n" => "n",
        "ny" => "nʲ",
        "h" => "h",
        "hy" => "hʲ",
        "f" => "ɸ",
        "b" => "b",
        "by" => "bʲ",
        "p" => "p",
        "py" => "pʲ",
        "m" => "m",
        "my" => "mʲ",
        "y" => "j",
        "r" => "ɾ",
        "ry" => "ɾʲ",
        "w" => "w",
        "v" => "v",
        "N" => "ɴ",
        "cl" => "ʔ",
        _ => return None,
    })
}

pub(crate) fn labels_to_ipa(labels: &[String], drop_devoiced_vowels: bool) -> String {
    let parsed: Vec<ParsedLabel<'_>> = labels
        .iter()
        .filter_map(|label| parse_label(label))
        .collect();
    let num_labels = parsed.len();
    let mut ipa = String::new();
    let mut has_speech = false;

    for (idx, current) in parsed.iter().enumerate() {
        let phone = fold_devoiced_vowel(current.phone, drop_devoiced_vowels);

        if phone == "sil" {
            if idx + 1 == num_labels && has_speech {
                ipa.push(if current.e3 == 1 { '?' } else { '.' });
            }
            continue;
        }

        if phone == "pau" {
            ipa.push(',');
            continue;
        }

        match openjtalk_to_ipa(phone) {
            Some(mapped) => {
                ipa.push_str(mapped);
                has_speech = true;
            }
            None => {
                tracing::warn!("No IPA mapping for OpenJTalk phone: {phone}");
                continue;
            }
        }

        let Some(next) = parsed.get(idx + 1) else {
            continue;
        };

        if current.a3 == 1 && next.a2 == 1 && is_mora_final_phone(phone) {
            ipa.push('#');
        } else if current.a1 == 0
            && next.a2 == current.a2 + 1
            && current.a2 != current.f1
        {
            ipa.push('↓');
        } else if current.a2 == 1 && next.a2 == 2 {
            ipa.push('↑');
        }
    }

    ipa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_sentences_keeps_closing_brackets_and_decimal_points() {
        assert_eq!(
            split_sentences("「はい。」元気ですか？ 1.5です。"),
            vec!["「はい。」", "元気ですか？", "1.5です。"]
        );
        assert!(split_sentences("   \n  ").is_empty());
    }

    #[test]
    fn labels_to_ipa_emits_prosody_and_folds_devoiced_vowels() {
        let labels = vec![
            "xx^xx-sil+ky=o/A:xx+xx+xx/E:xx_xx!xx_xx-xx/F:xx_xx".to_string(),
            "xx^sil-ky+o=o/A:0+1+2/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "sil^ky-o+o=w/A:0+1+2/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "ky^o-o+w=a/A:1+2+1/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "o^o-w+a=pau/A:0+1+1/E:xx_xx!xx_xx-xx/F:1_1".to_string(),
            "o^w-a+pau=s/A:0+1+1/E:xx_xx!xx_xx-xx/F:1_1".to_string(),
            "w^a-pau+s=U/A:xx+xx+xx/E:xx_xx!xx_xx-xx/F:xx_xx".to_string(),
            "a^pau-s+U=sil/A:0+1+1/E:1_1!0_xx-xx/F:1_1".to_string(),
            "pau^s-U+sil=xx/A:0+1+1/E:1_1!0_xx-xx/F:1_1".to_string(),
            "s^U-sil+xx=xx/A:xx+xx+xx/E:1_1!1_xx-xx/F:xx_xx".to_string(),
        ];
        assert_eq!(labels_to_ipa(&labels, true), "kʲo↓o#wa,sɯ?");

        let rising = vec![
            "xx^xx-sil+k=a/A:xx+xx+xx/E:xx_xx!xx_xx-xx/F:xx_xx".to_string(),
            "xx^sil-k+a=n/A:-1+1+2/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "sil^k-a+n=a/A:-1+1+2/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "k^a-n+a=sil/A:0+2+1/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "a^n-a+sil=xx/A:0+2+1/E:xx_xx!xx_xx-xx/F:2_1".to_string(),
            "n^a-sil+xx=xx/A:xx+xx+xx/E:2_1!0_xx-xx/F:xx_xx".to_string(),
        ];
        assert_eq!(labels_to_ipa(&rising, true), "ka↑na.");
    }

    #[test]
    fn labels_to_ipa_skips_trailing_sil_when_there_are_no_speech_phones() {
        let labels = vec![
            "xx^xx-sil+sil=xx/A:xx+xx+xx/E:xx_xx!xx_xx-xx/F:xx_xx".to_string(),
            "xx^sil-sil+xx=xx/A:xx+xx+xx/E:xx_xx!0_xx-xx/F:xx_xx".to_string(),
        ];
        assert_eq!(labels_to_ipa(&labels, true), "");
    }

    #[test]
    fn unset_and_missing_dictionary_errors_name_env_variable() {
        let err = JapaneseG2p::from_env_value(None).err().map(|e| e.to_string());
        assert!(err.is_some_and(|m| m.contains(JAPANESE_DICT_ENV)));

        let err = JapaneseG2p::from_env_value(Some("/nonexistent/ja-dict".into()))
            .err()
            .map(|e| e.to_string());
        assert!(err
            .is_some_and(|m| m.contains(JAPANESE_DICT_ENV) && m.contains("/nonexistent/ja-dict")));
    }

    #[test]
    fn real_dictionary_phonemizes_japanese_sentences_with_prosody() {
        let Some(value) = std::env::var_os(JAPANESE_DICT_ENV) else {
            return;
        };
        let g2p = JapaneseG2p::from_env_value(Some(value))
            .expect("DENGJEN_JA_DICT_DIR is set but failed to load");
        let phones = g2p.phonemize("今日は雨ですか？").unwrap();
        assert_eq!(phones.len(), 1);
        assert!(phones[0].ends_with('?'));
        assert!(phones[0].contains('↑') || phones[0].contains('↓') || phones[0].contains('#'));
    }

    #[test]
    fn real_dictionary_handles_known_and_unknown_latin_words() {
        let Some(value) = std::env::var_os(JAPANESE_DICT_ENV) else {
            return;
        };
        let g2p = JapaneseG2p::from_env_value(Some(value))
            .expect("DENGJEN_JA_DICT_DIR is set but failed to load");
        assert_eq!(g2p.preprocess_foreign_words("please"), "please");
        for word in ["pause", "generator", "blanco", "NVDA"] {
            let prepared = g2p.preprocess_foreign_words(word);
            let phones = g2p.phonemize(word).unwrap();
            println!("{word} -> prepared=`{prepared}` -> ipa={phones:?}");
            assert_ne!(prepared.as_ref(), word, "word `{word}` was not converted");
            assert_eq!(phones.len(), 1, "word `{word}` produced {phones:?}");
            assert!(!phones[0].is_empty(), "word `{word}` produced empty IPA");
        }
    }
}
