pub(crate) fn foreign_word_to_katakana(word: &str) -> String {
    if is_spelled_acronym(word) {
        return letters_to_katakana(word);
    }
    #[cfg(feature = "espeak")]
    if let Some(kana) = espeak_english_to_katakana(word) {
        if !kana.is_empty() {
            return kana;
        }
    }
    romaji_to_katakana(word)
}

#[cfg(feature = "espeak")]
fn espeak_english_to_katakana(word: &str) -> Option<String> {
    let sentences =
        dengjen_espeak_phonemizer::text_to_phonemes(word, "en-us", None, true, false).ok()?;
    let joined = sentences.concat();
    let kana = ipa_to_katakana(&joined);
    (!kana.is_empty()).then_some(kana)
}

fn has_ascii_vowel(word: &str) -> bool {
    word.bytes()
        .any(|b| matches!(b.to_ascii_lowercase(), b'a' | b'e' | b'i' | b'o' | b'u' | b'y'))
}

fn is_valid_english_onset_pair(a: u8, b: u8) -> bool {
    matches!(
        (a, b),
        (b'b' | b'c' | b'd' | b'f' | b'g' | b'k' | b'p' | b's' | b't', b'r' | b'l')
            | (b's', b'c' | b'k' | b'm' | b'n' | b'p' | b't' | b'w')
            | (b'c' | b'p' | b's' | b't' | b'w', b'h')
            | (b'k' | b'g' | b'p', b'n')
            | (b'p', b's')
            | (b't' | b'd', b'w')
            | (b'g', b'w')
            | (b'q', b'u')
    )
}

fn is_spelled_acronym(word: &str) -> bool {
    if word.contains('\'') || word.len() < 2 {
        return false;
    }
    if !has_ascii_vowel(word) {
        return true;
    }
    let is_all_upper = word.bytes().all(|b| b.is_ascii_uppercase());
    if !is_all_upper || word.len() > 5 {
        return false;
    }
    let bytes = word.as_bytes();
    match (bytes.first().copied(), bytes.get(1).copied()) {
        (Some(a), Some(b)) => {
            let a_low = a.to_ascii_lowercase();
            let b_low = b.to_ascii_lowercase();
            let a_is_cons = a_low.is_ascii_alphabetic()
                && !matches!(a_low, b'a' | b'e' | b'i' | b'o' | b'u' | b'y');
            let b_is_cons = b_low.is_ascii_alphabetic()
                && !matches!(b_low, b'a' | b'e' | b'i' | b'o' | b'u' | b'y');
            a_is_cons && b_is_cons && !is_valid_english_onset_pair(a_low, b_low)
        }
        _ => false,
    }
}

fn letters_to_katakana(word: &str) -> String {
    let mut out = String::new();
    for ch in word.chars() {
        let kana = match ch.to_ascii_uppercase() {
            'A' => "エー",
            'B' => "ビー",
            'C' => "シー",
            'D' => "ディー",
            'E' => "イー",
            'F' => "エフ",
            'G' => "ジー",
            'H' => "エイチ",
            'I' => "アイ",
            'J' => "ジェイ",
            'K' => "ケイ",
            'L' => "エル",
            'M' => "エム",
            'N' => "エヌ",
            'O' => "オー",
            'P' => "ピー",
            'Q' => "キュー",
            'R' => "アール",
            'S' => "エス",
            'T' => "ティー",
            'U' => "ユー",
            'V' => "ブイ",
            'W' => "ダブリュー",
            'X' => "エックス",
            'Y' => "ワイ",
            'Z' => "ゼット",
            _ => "",
        };
        out.push_str(kana);
    }
    out
}

#[cfg(any(feature = "espeak", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VowelKind {
    A,
    I,
    U,
    E,
    O,
    Ae,
}

#[cfg(any(feature = "espeak", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VowelShape {
    Short { kind: VowelKind, stressed: bool },
    Long(VowelKind),
    Diphthong(VowelKind, VowelKind),
}

#[cfg(any(feature = "espeak", test))]
impl VowelShape {
    fn base(self) -> VowelKind {
        match self {
            Self::Short { kind, .. } | Self::Long(kind) | Self::Diphthong(kind, _) => kind,
        }
    }

    fn tail(self) -> &'static str {
        match self {
            Self::Short { .. } => "",
            Self::Long(_) => "ー",
            Self::Diphthong(_, second) => standalone_vowel(second),
        }
    }

    fn ends_with_long_mark(self) -> bool {
        matches!(self, Self::Long(_))
    }

    fn is_stressed_short(self) -> bool {
        matches!(self, Self::Short { stressed: true, .. })
    }

    fn is_unstressed_schwa(self) -> bool {
        matches!(
            self,
            Self::Short {
                kind: VowelKind::A,
                stressed: false
            }
        )
    }
}

#[cfg(any(feature = "espeak", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cons {
    P,
    B,
    T,
    D,
    K,
    G,
    F,
    V,
    Th,
    Dh,
    S,
    Z,
    Sh,
    Zh,
    Ch,
    J,
    Ts,
    H,
    M,
    N,
    Ng,
    L,
    R,
    Y,
    W,
}

#[cfg(any(feature = "espeak", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IpaToken {
    Cons(Cons),
    Vowel(VowelShape),
    Syllabic(Cons),
    Break,
}

#[cfg(any(feature = "espeak", test))]
fn standalone_vowel(kind: VowelKind) -> &'static str {
    match kind {
        VowelKind::A | VowelKind::Ae => "ア",
        VowelKind::I => "イ",
        VowelKind::U => "ウ",
        VowelKind::E => "エ",
        VowelKind::O => "オ",
    }
}

#[cfg(any(feature = "espeak", test))]
fn is_rhotic_before_r_and_vowel(chars: &[char], next_idx: usize) -> bool {
    let Some(&r_ch) = chars.get(next_idx) else {
        return false;
    };
    if !matches!(r_ch, 'ɹ' | 'r') {
        return false;
    }
    let mut after_r = next_idx + 1;
    while matches!(chars.get(after_r), Some('ˈ' | 'ˌ')) {
        after_r += 1;
    }
    matches!(
        chars.get(after_r),
        Some(
            'a' | 'e'
                | 'i'
                | 'o'
                | 'u'
                | 'ɪ'
                | 'ɛ'
                | 'æ'
                | 'ɑ'
                | 'ɒ'
                | 'ɔ'
                | 'ʌ'
                | 'ə'
                | 'ʊ'
                | 'ɜ'
                | 'ɚ'
                | 'ᵻ'
        )
    )
}

#[cfg(any(feature = "espeak", test))]
fn is_tie_or_nonsyllabic(ch: char) -> bool {
    matches!(ch, '\u{0361}' | '\u{032F}')
}

#[cfg(any(feature = "espeak", test))]
fn parse_ipa(ipa: &str, mut on_unknown: impl FnMut(char)) -> Vec<IpaToken> {
    let chars: Vec<char> = ipa
        .chars()
        .filter(|&c| !is_tie_or_nonsyllabic(c))
        .collect();
    let mut tokens = Vec::with_capacity(chars.len());
    let mut i = 0;
    let mut next_stressed = false;

    while let Some(&ch) = chars.get(i) {
        if ch == 'ˈ' {
            next_stressed = true;
            i += 1;
            continue;
        }
        if matches!(ch, 'ˌ' | '.' | ',' | '?' | '!' | ';' | ':') {
            next_stressed = false;
            i += 1;
            continue;
        }
        if ch == ' ' {
            tokens.push(IpaToken::Break);
            next_stressed = false;
            i += 1;
            continue;
        }

        match (chars.get(i), chars.get(i + 1), chars.get(i + 2)) {
            (Some('a'), Some('ɪ'), Some('ə' | 'ɚ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::A,
                    VowelKind::I,
                )));
                tokens.push(IpaToken::Vowel(VowelShape::Long(VowelKind::A)));
                next_stressed = false;
                i += 3;
                continue;
            }
            (Some('a'), Some('ʊ'), Some('ə' | 'ɚ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::A,
                    VowelKind::U,
                )));
                tokens.push(IpaToken::Vowel(VowelShape::Long(VowelKind::A)));
                next_stressed = false;
                i += 3;
                continue;
            }
            (Some('a'), Some('ɪ'), Some('ʊ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::A,
                    VowelKind::U,
                )));
                next_stressed = false;
                i += 3;
                continue;
            }
            _ => {}
        }

        match (chars.get(i), chars.get(i + 1)) {
            (Some('a'), Some('ɪ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::A,
                    VowelKind::I,
                )));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('a'), Some('ʊ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::A,
                    VowelKind::U,
                )));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('ɔ'), Some('ɪ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::O,
                    VowelKind::I,
                )));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('e'), Some('ɪ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Diphthong(
                    VowelKind::E,
                    VowelKind::I,
                )));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('o' | 'ə'), Some('ʊ')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Long(VowelKind::O)));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('t'), Some('ʃ')) => {
                tokens.push(IpaToken::Cons(Cons::Ch));
                i += 2;
                continue;
            }
            (Some('d'), Some('ʒ')) => {
                tokens.push(IpaToken::Cons(Cons::J));
                i += 2;
                continue;
            }
            (Some('t'), Some('s')) => {
                tokens.push(IpaToken::Cons(Cons::Ts));
                i += 2;
                continue;
            }
            (Some('l' | 'ɫ'), Some('\u{0329}')) => {
                tokens.push(IpaToken::Syllabic(Cons::L));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('n'), Some('\u{0329}')) => {
                tokens.push(IpaToken::Syllabic(Cons::N));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('m'), Some('\u{0329}')) => {
                tokens.push(IpaToken::Syllabic(Cons::M));
                next_stressed = false;
                i += 2;
                continue;
            }
            (Some('ɹ' | 'r'), Some('\u{0329}')) => {
                tokens.push(IpaToken::Vowel(VowelShape::Long(VowelKind::A)));
                next_stressed = false;
                i += 2;
                continue;
            }
            _ => {}
        }

        if matches!(ch, 'ɚ' | 'ɝ') {
            tokens.push(IpaToken::Vowel(
                if is_rhotic_before_r_and_vowel(&chars, i + 1) {
                    VowelShape::Short {
                        kind: VowelKind::A,
                        stressed: false,
                    }
                } else {
                    VowelShape::Long(VowelKind::A)
                },
            ));
            next_stressed = false;
            i += 1;
            continue;
        }

        let vowel_kind = match ch {
            'ɪ' | 'i' | 'ᵻ' => Some((VowelKind::I, next_stressed)),
            'ɛ' | 'e' => Some((VowelKind::E, next_stressed)),
            'æ' => Some((VowelKind::Ae, next_stressed)),
            'ɑ' | 'ɒ' | 'ɔ' | 'o' => Some((VowelKind::O, next_stressed)),
            'ʌ' | 'ɜ' | 'ɐ' | 'a' => Some((VowelKind::A, next_stressed)),
            'ə' => Some((VowelKind::A, false)),
            'ʊ' | 'u' => Some((VowelKind::U, next_stressed)),
            _ => None,
        };
        if let Some((kind, stressed)) = vowel_kind {
            let has_length_mark = chars.get(i + 1) == Some(&'ː');
            tokens.push(IpaToken::Vowel(if has_length_mark {
                VowelShape::Long(kind)
            } else {
                VowelShape::Short { kind, stressed }
            }));
            next_stressed = false;
            i += if has_length_mark { 2 } else { 1 };
            continue;
        }

        let cons = match ch {
            'p' => Some(Cons::P),
            'b' => Some(Cons::B),
            't' | 'ɾ' | 'ʔ' => Some(Cons::T),
            'd' => Some(Cons::D),
            'k' | 'x' => Some(Cons::K),
            'ɡ' | 'g' => Some(Cons::G),
            'f' => Some(Cons::F),
            'v' => Some(Cons::V),
            'θ' => Some(Cons::Th),
            'ð' => Some(Cons::Dh),
            's' => Some(Cons::S),
            'z' => Some(Cons::Z),
            'ʃ' => Some(Cons::Sh),
            'ʒ' => Some(Cons::Zh),
            'h' | 'ç' => Some(Cons::H),
            'm' => Some(Cons::M),
            'n' => Some(Cons::N),
            'ŋ' => Some(Cons::Ng),
            'l' | 'ɫ' | 'ɬ' => Some(Cons::L),
            'ɹ' | 'r' | 'ʁ' => Some(Cons::R),
            'j' => Some(Cons::Y),
            'w' | 'ʍ' => Some(Cons::W),
            _ => None,
        };

        if let Some(c) = cons {
            tokens.push(IpaToken::Cons(c));
        } else {
            on_unknown(ch);
        }
        i += 1;
    }

    tokens
}

#[cfg(any(feature = "espeak", test))]
fn cv_kana(cons: Cons, vowel: VowelKind) -> &'static str {
    match (cons, vowel) {
        (Cons::K, VowelKind::Ae) => "キャ",
        (Cons::K, VowelKind::A) => "カ",
        (Cons::K, VowelKind::I) => "キ",
        (Cons::K, VowelKind::U) => "ク",
        (Cons::K, VowelKind::E) => "ケ",
        (Cons::K, VowelKind::O) => "コ",

        (Cons::G, VowelKind::Ae) => "ギャ",
        (Cons::G, VowelKind::A) => "ガ",
        (Cons::G, VowelKind::I) => "ギ",
        (Cons::G, VowelKind::U) => "グ",
        (Cons::G, VowelKind::E) => "ゲ",
        (Cons::G, VowelKind::O) => "ゴ",

        (Cons::Ng, VowelKind::A | VowelKind::Ae) => "ンガ",
        (Cons::Ng, VowelKind::I) => "ンギ",
        (Cons::Ng, VowelKind::U) => "ング",
        (Cons::Ng, VowelKind::E) => "ンゲ",
        (Cons::Ng, VowelKind::O) => "ンゴ",

        (Cons::S | Cons::Th, VowelKind::A | VowelKind::Ae) => "サ",
        (Cons::S | Cons::Th, VowelKind::I) => "シ",
        (Cons::S | Cons::Th, VowelKind::U) => "ス",
        (Cons::S | Cons::Th, VowelKind::E) => "セ",
        (Cons::S | Cons::Th, VowelKind::O) => "ソ",

        (Cons::Z | Cons::Dh, VowelKind::A | VowelKind::Ae) => "ザ",
        (Cons::Z | Cons::Dh, VowelKind::I) => "ジ",
        (Cons::Z | Cons::Dh, VowelKind::U) => "ズ",
        (Cons::Z | Cons::Dh, VowelKind::E) => "ゼ",
        (Cons::Z | Cons::Dh, VowelKind::O) => "ゾ",

        (Cons::Sh, VowelKind::A | VowelKind::Ae) => "シャ",
        (Cons::Sh, VowelKind::I) => "シ",
        (Cons::Sh, VowelKind::U) => "シュ",
        (Cons::Sh, VowelKind::E) => "シェ",
        (Cons::Sh, VowelKind::O) => "ショ",

        (Cons::Zh | Cons::J, VowelKind::A | VowelKind::Ae) => "ジャ",
        (Cons::Zh | Cons::J, VowelKind::I) => "ジ",
        (Cons::Zh | Cons::J, VowelKind::U) => "ジュ",
        (Cons::Zh | Cons::J, VowelKind::E) => "ジェ",
        (Cons::Zh | Cons::J, VowelKind::O) => "ジョ",

        (Cons::Ch, VowelKind::A | VowelKind::Ae) => "チャ",
        (Cons::Ch, VowelKind::I) => "チ",
        (Cons::Ch, VowelKind::U) => "チュ",
        (Cons::Ch, VowelKind::E) => "チェ",
        (Cons::Ch, VowelKind::O) => "チョ",

        (Cons::Ts, VowelKind::A | VowelKind::Ae) => "ツァ",
        (Cons::Ts, VowelKind::I) => "ツィ",
        (Cons::Ts, VowelKind::U) => "ツ",
        (Cons::Ts, VowelKind::E) => "ツェ",
        (Cons::Ts, VowelKind::O) => "ツォ",

        (Cons::T, VowelKind::A | VowelKind::Ae) => "タ",
        (Cons::T, VowelKind::I) => "ティ",
        (Cons::T, VowelKind::U) => "トゥ",
        (Cons::T, VowelKind::E) => "テ",
        (Cons::T, VowelKind::O) => "ト",

        (Cons::D, VowelKind::A | VowelKind::Ae) => "ダ",
        (Cons::D, VowelKind::I) => "ディ",
        (Cons::D, VowelKind::U) => "ドゥ",
        (Cons::D, VowelKind::E) => "デ",
        (Cons::D, VowelKind::O) => "ド",

        (Cons::N, VowelKind::A | VowelKind::Ae) => "ナ",
        (Cons::N, VowelKind::I) => "ニ",
        (Cons::N, VowelKind::U) => "ヌ",
        (Cons::N, VowelKind::E) => "ネ",
        (Cons::N, VowelKind::O) => "ノ",

        (Cons::H, VowelKind::A | VowelKind::Ae) => "ハ",
        (Cons::H, VowelKind::I) => "ヒ",
        (Cons::H, VowelKind::U) => "フ",
        (Cons::H, VowelKind::E) => "ヘ",
        (Cons::H, VowelKind::O) => "ホ",

        (Cons::F, VowelKind::A | VowelKind::Ae) => "ファ",
        (Cons::F, VowelKind::I) => "フィ",
        (Cons::F, VowelKind::U) => "フ",
        (Cons::F, VowelKind::E) => "フェ",
        (Cons::F, VowelKind::O) => "フォ",

        (Cons::B | Cons::V, VowelKind::A | VowelKind::Ae) => "バ",
        (Cons::B | Cons::V, VowelKind::I) => "ビ",
        (Cons::B | Cons::V, VowelKind::U) => "ブ",
        (Cons::B | Cons::V, VowelKind::E) => "ベ",
        (Cons::B | Cons::V, VowelKind::O) => "ボ",

        (Cons::P, VowelKind::A | VowelKind::Ae) => "パ",
        (Cons::P, VowelKind::I) => "ピ",
        (Cons::P, VowelKind::U) => "プ",
        (Cons::P, VowelKind::E) => "ペ",
        (Cons::P, VowelKind::O) => "ポ",

        (Cons::M, VowelKind::A | VowelKind::Ae) => "マ",
        (Cons::M, VowelKind::I) => "ミ",
        (Cons::M, VowelKind::U) => "ム",
        (Cons::M, VowelKind::E) => "メ",
        (Cons::M, VowelKind::O) => "モ",

        (Cons::L | Cons::R, VowelKind::A | VowelKind::Ae) => "ラ",
        (Cons::L | Cons::R, VowelKind::I) => "リ",
        (Cons::L | Cons::R, VowelKind::U) => "ル",
        (Cons::L | Cons::R, VowelKind::E) => "レ",
        (Cons::L | Cons::R, VowelKind::O) => "ロ",

        (Cons::Y, VowelKind::A | VowelKind::Ae) => "ヤ",
        (Cons::Y, VowelKind::I) => "イ",
        (Cons::Y, VowelKind::U) => "ユ",
        (Cons::Y, VowelKind::E) => "イェ",
        (Cons::Y, VowelKind::O) => "ヨ",

        (Cons::W, VowelKind::A | VowelKind::Ae) => "ワ",
        (Cons::W, VowelKind::I) => "ウィ",
        (Cons::W, VowelKind::U) => "ウ",
        (Cons::W, VowelKind::E) => "ウェ",
        (Cons::W, VowelKind::O) => "ウォ",
    }
}

#[cfg(any(feature = "espeak", test))]
fn cy_kana(cons: Cons, vowel: VowelKind) -> Option<&'static str> {
    Some(match (cons, vowel) {
        (Cons::K, VowelKind::A | VowelKind::Ae) => "キャ",
        (Cons::K, VowelKind::U) => "キュ",
        (Cons::K, VowelKind::O) => "キョ",
        (Cons::G, VowelKind::A | VowelKind::Ae) => "ギャ",
        (Cons::G, VowelKind::U) => "ギュ",
        (Cons::G, VowelKind::O) => "ギョ",
        (Cons::S | Cons::Sh | Cons::Th, VowelKind::A | VowelKind::Ae) => "シャ",
        (Cons::S | Cons::Sh | Cons::Th, VowelKind::U) => "シュ",
        (Cons::S | Cons::Sh | Cons::Th, VowelKind::O) => "ショ",
        (Cons::Z | Cons::J | Cons::Zh | Cons::Dh, VowelKind::A | VowelKind::Ae) => "ジャ",
        (Cons::Z | Cons::J | Cons::Zh | Cons::Dh, VowelKind::U) => "ジュ",
        (Cons::Z | Cons::J | Cons::Zh | Cons::Dh, VowelKind::O) => "ジョ",
        (Cons::T | Cons::Ch, VowelKind::A | VowelKind::Ae) => "チャ",
        (Cons::T | Cons::Ch, VowelKind::U) => "チュ",
        (Cons::T | Cons::Ch, VowelKind::O) => "チョ",
        (Cons::D, VowelKind::A | VowelKind::Ae) => "ジャ",
        (Cons::D, VowelKind::U) => "デュ",
        (Cons::D, VowelKind::O) => "ジョ",
        (Cons::N, VowelKind::A | VowelKind::Ae) => "ニャ",
        (Cons::N, VowelKind::U) => "ニュ",
        (Cons::N, VowelKind::O) => "ニョ",
        (Cons::H, VowelKind::A | VowelKind::Ae) => "ヒャ",
        (Cons::H, VowelKind::U) => "ヒュ",
        (Cons::H, VowelKind::O) => "ヒョ",
        (Cons::F, VowelKind::A | VowelKind::Ae) => "フャ",
        (Cons::F, VowelKind::U) => "フュ",
        (Cons::F, VowelKind::O) => "フョ",
        (Cons::B | Cons::V, VowelKind::A | VowelKind::Ae) => "ビャ",
        (Cons::B | Cons::V, VowelKind::U) => "ビュ",
        (Cons::B | Cons::V, VowelKind::O) => "ビョ",
        (Cons::P, VowelKind::A | VowelKind::Ae) => "ピャ",
        (Cons::P, VowelKind::U) => "ピュ",
        (Cons::P, VowelKind::O) => "ピョ",
        (Cons::M, VowelKind::A | VowelKind::Ae) => "ミャ",
        (Cons::M, VowelKind::U) => "ミュ",
        (Cons::M, VowelKind::O) => "ミョ",
        (Cons::L | Cons::R, VowelKind::A | VowelKind::Ae) => "リャ",
        (Cons::L | Cons::R, VowelKind::U) => "リュ",
        (Cons::L | Cons::R, VowelKind::O) => "リョ",
        _ => return None,
    })
}

#[cfg(any(feature = "espeak", test))]
fn coda_kana(cons: Cons, next: Option<&IpaToken>) -> &'static str {
    match cons {
        Cons::P => "プ",
        Cons::B | Cons::V => "ブ",
        Cons::T => "ト",
        Cons::D => "ド",
        Cons::K => "ク",
        Cons::G => "グ",
        Cons::F => "フ",
        Cons::S | Cons::Th => "ス",
        Cons::Z | Cons::Dh => "ズ",
        Cons::Sh => "シュ",
        Cons::Zh | Cons::J => "ジ",
        Cons::Ch => "チ",
        Cons::Ts => "ツ",
        Cons::H => "フ",
        Cons::M => match next {
            Some(IpaToken::Cons(Cons::P | Cons::B | Cons::M)) => "ン",
            _ => "ム",
        },
        Cons::N => "ン",
        Cons::Ng => match next {
            Some(IpaToken::Cons(Cons::K | Cons::G)) => "ン",
            _ => "ング",
        },
        Cons::L | Cons::R => "ル",
        Cons::Y => "イ",
        Cons::W => "ウ",
    }
}

#[cfg(any(feature = "espeak", test))]
fn should_geminate_after_stressed_short(tokens: &[IpaToken], after_vowel_idx: usize) -> bool {
    let Some(IpaToken::Cons(c)) = tokens.get(after_vowel_idx) else {
        return false;
    };
    if !matches!(c, Cons::P | Cons::T | Cons::K | Cons::Ch) {
        return false;
    }
    match tokens.get(after_vowel_idx + 1) {
        None | Some(IpaToken::Break) => true,
        Some(IpaToken::Cons(Cons::S)) if *c == Cons::K => {
            matches!(tokens.get(after_vowel_idx + 2), None | Some(IpaToken::Break))
        }
        _ => false,
    }
}

#[cfg(any(feature = "espeak", test))]
fn is_vowel_or_glide(token: Option<&IpaToken>) -> bool {
    matches!(token, Some(IpaToken::Vowel(_)))
}

#[cfg(any(feature = "espeak", test))]
pub(crate) fn ipa_to_katakana(ipa: &str) -> String {
    let tokens = parse_ipa(ipa, |ch| {
        tracing::debug!("Unmapped en-us IPA char {ch:?} in {ipa:?}");
    });
    let mut out = String::new();
    let mut i = 0;

    while let Some(&tok) = tokens.get(i) {
        match tok {
            IpaToken::Break => {
                i += 1;
            }
            IpaToken::Syllabic(c) => {
                out.push_str(coda_kana(c, tokens.get(i + 1)));
                i += 1;
            }
            IpaToken::Cons(c) => {
                if let (Some(IpaToken::Cons(Cons::Y)), Some(IpaToken::Vowel(v))) =
                    (tokens.get(i + 1), tokens.get(i + 2))
                {
                    if let Some(pal) = cy_kana(c, v.base()) {
                        out.push_str(pal);
                        out.push_str(v.tail());
                        let mut next_idx = i + 3;
                        if matches!(tokens.get(next_idx), Some(IpaToken::Cons(Cons::R)))
                            && !is_vowel_or_glide(tokens.get(next_idx + 1))
                        {
                            if !v.ends_with_long_mark() {
                                out.push('ー');
                            }
                            next_idx += 1;
                        } else if v.is_stressed_short()
                            && should_geminate_after_stressed_short(&tokens, next_idx)
                        {
                            out.push('ッ');
                        }
                        i = next_idx;
                        continue;
                    }
                }

                if let Some(&IpaToken::Vowel(v)) = tokens.get(i + 1) {
                    if v.is_unstressed_schwa()
                        && matches!(tokens.get(i + 2), Some(IpaToken::Cons(Cons::L)))
                        && !is_vowel_or_glide(tokens.get(i + 3))
                    {
                        out.push_str(coda_kana(c, Some(&IpaToken::Cons(Cons::L))));
                        out.push('ル');
                        i += 3;
                        continue;
                    }

                    out.push_str(cv_kana(c, v.base()));
                    out.push_str(v.tail());
                    let mut next_idx = i + 2;
                    if matches!(tokens.get(next_idx), Some(IpaToken::Cons(Cons::R)))
                        && !is_vowel_or_glide(tokens.get(next_idx + 1))
                    {
                        if !v.ends_with_long_mark() {
                            out.push('ー');
                        }
                        next_idx += 1;
                    } else if v.is_stressed_short()
                        && should_geminate_after_stressed_short(&tokens, next_idx)
                    {
                        out.push('ッ');
                    }
                    i = next_idx;
                } else {
                    out.push_str(coda_kana(c, tokens.get(i + 1)));
                    i += 1;
                }
            }
            IpaToken::Vowel(v) => {
                out.push_str(standalone_vowel(v.base()));
                out.push_str(v.tail());
                let mut next_idx = i + 1;
                if matches!(tokens.get(next_idx), Some(IpaToken::Cons(Cons::R)))
                    && !is_vowel_or_glide(tokens.get(next_idx + 1))
                {
                    if !v.ends_with_long_mark() {
                        out.push('ー');
                    }
                    next_idx += 1;
                } else if v.is_stressed_short()
                    && should_geminate_after_stressed_short(&tokens, next_idx)
                {
                    out.push('ッ');
                }
                i = next_idx;
            }
        }
    }

    out
}

pub(crate) fn romaji_to_katakana(word: &str) -> String {
    let chars: Vec<char> = word
        .chars()
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let mut out = String::new();
    let mut i = 0;

    while let Some(&c0) = chars.get(i) {
        let c1 = chars.get(i + 1).copied();
        let c2 = chars.get(i + 2).copied();

        if let Some(kana) = match_romaji_trigraph(c0, c1, c2) {
            out.push_str(kana);
            i += 3;
            continue;
        }
        if let Some(kana) = match_romaji_digraph(c0, c1) {
            out.push_str(kana);
            i += 2;
            continue;
        }

        if let Some(next) = c1 {
            if c0 == next
                && !matches!(c0, 'a' | 'e' | 'i' | 'o' | 'u' | 'n' | 'l' | 'r')
            {
                out.push('ッ');
                i += 1;
                continue;
            }
        }

        if c0 == 'n' && !matches!(c1, Some('a' | 'i' | 'u' | 'e' | 'o' | 'y')) {
            out.push('ン');
            i += 1;
            continue;
        }
        if c0 == 'm' && matches!(c1, Some('p' | 'b' | 'm')) {
            out.push('ン');
            i += 1;
            continue;
        }

        let single = match c0 {
            'a' => "ア",
            'i' | 'y' => "イ",
            'u' => "ウ",
            'e' => "エ",
            'o' => "オ",
            'b' | 'v' => "ブ",
            'c' | 'k' | 'q' => "ク",
            'd' => "ド",
            'f' | 'h' => "フ",
            'g' => "グ",
            'j' => "ジ",
            'l' | 'r' => "ル",
            'm' => "ム",
            'n' => "ン",
            'p' => "プ",
            's' => "ス",
            't' => "ト",
            'w' => "ウ",
            'x' => "クス",
            'z' => "ズ",
            _ => "",
        };
        out.push_str(single);
        i += 1;
    }

    out
}

fn match_romaji_trigraph(c0: char, c1: Option<char>, c2: Option<char>) -> Option<&'static str> {
    let (Some(c1), Some(c2)) = (c1, c2) else {
        return None;
    };
    Some(match (c0, c1, c2) {
        ('k', 'y', 'a') => "キャ",
        ('k', 'y', 'u') => "キュ",
        ('k', 'y', 'o') => "キョ",
        ('g', 'y', 'a') => "ギャ",
        ('g', 'y', 'u') => "ギュ",
        ('g', 'y', 'o') => "ギョ",
        ('s', 'h', 'a') => "シャ",
        ('s', 'h', 'i') => "シ",
        ('s', 'h', 'u') => "シュ",
        ('s', 'h', 'e') => "シェ",
        ('s', 'h', 'o') => "ショ",
        ('c', 'h', 'a') => "チャ",
        ('c', 'h', 'i') => "チ",
        ('c', 'h', 'u') => "チュ",
        ('c', 'h', 'e') => "チェ",
        ('c', 'h', 'o') => "チョ",
        ('t', 's', 'u') => "ツ",
        ('n', 'y', 'a') => "ニャ",
        ('n', 'y', 'u') => "ニュ",
        ('n', 'y', 'o') => "ニョ",
        ('h', 'y', 'a') => "ヒャ",
        ('h', 'y', 'u') => "ヒュ",
        ('h', 'y', 'o') => "ヒョ",
        ('m', 'y', 'a') => "ミャ",
        ('m', 'y', 'u') => "ミュ",
        ('m', 'y', 'o') => "ミョ",
        ('r' | 'l', 'y', 'a') => "リャ",
        ('r' | 'l', 'y', 'u') => "リュ",
        ('r' | 'l', 'y', 'o') => "リョ",
        ('b' | 'v', 'y', 'a') => "ビャ",
        ('b' | 'v', 'y', 'u') => "ビュ",
        ('b' | 'v', 'y', 'o') => "ビョ",
        ('p', 'y', 'a') => "ピャ",
        ('p', 'y', 'u') => "ピュ",
        ('p', 'y', 'o') => "ピョ",
        ('j', 'y', 'a') => "ジャ",
        ('j', 'y', 'u') => "ジュ",
        ('j', 'y', 'o') => "ジョ",
        ('d', 'y', 'a') => "ヂャ",
        ('d', 'y', 'u') => "デュ",
        ('d', 'y', 'o') => "ヂョ",
        ('t', 'h', 'a') => "サ",
        ('t', 'h', 'i') => "シ",
        ('t', 'h', 'u') => "ス",
        ('t', 'h', 'e') => "セ",
        ('t', 'h', 'o') => "ソ",
        ('p', 'h', 'a') => "ファ",
        ('p', 'h', 'i') => "フィ",
        ('p', 'h', 'u') => "フ",
        ('p', 'h', 'e') => "フェ",
        ('p', 'h', 'o') => "フォ",
        ('q', 'u', 'a') => "クァ",
        ('q', 'u', 'i') => "クィ",
        ('q', 'u', 'e') => "クェ",
        ('q', 'u', 'o') => "クォ",
        _ => return None,
    })
}

fn match_romaji_digraph(c0: char, c1: Option<char>) -> Option<&'static str> {
    let c1 = c1?;
    Some(match (c0, c1) {
        ('k', 'a') => "カ",
        ('k', 'i') => "キ",
        ('k', 'u') => "ク",
        ('k', 'e') => "ケ",
        ('k', 'o') => "コ",
        ('c', 'a') => "カ",
        ('c', 'i') => "シ",
        ('c', 'u') => "ク",
        ('c', 'e') => "セ",
        ('c', 'o') => "コ",
        ('g', 'a') => "ガ",
        ('g', 'i') => "ギ",
        ('g', 'u') => "グ",
        ('g', 'e') => "ゲ",
        ('g', 'o') => "ゴ",
        ('s', 'a') => "サ",
        ('s', 'i') => "シ",
        ('s', 'u') => "ス",
        ('s', 'e') => "セ",
        ('s', 'o') => "ソ",
        ('z', 'a') => "ザ",
        ('z', 'i') => "ジ",
        ('z', 'u') => "ズ",
        ('z', 'e') => "ゼ",
        ('z', 'o') => "ゾ",
        ('j', 'a') => "ジャ",
        ('j', 'i') => "ジ",
        ('j', 'u') => "ジュ",
        ('j', 'e') => "ジェ",
        ('j', 'o') => "ジョ",
        ('t', 'a') => "タ",
        ('t', 'i') => "ティ",
        ('t', 'u') => "トゥ",
        ('t', 'e') => "テ",
        ('t', 'o') => "ト",
        ('d', 'a') => "ダ",
        ('d', 'i') => "ディ",
        ('d', 'u') => "ドゥ",
        ('d', 'e') => "デ",
        ('d', 'o') => "ド",
        ('n', 'a') => "ナ",
        ('n', 'i') => "ニ",
        ('n', 'u') => "ヌ",
        ('n', 'e') => "ネ",
        ('n', 'o') => "ノ",
        ('h', 'a') => "ハ",
        ('h', 'i') => "ヒ",
        ('h', 'u') => "フ",
        ('h', 'e') => "ヘ",
        ('h', 'o') => "ホ",
        ('f', 'a') => "ファ",
        ('f', 'i') => "フィ",
        ('f', 'u') => "フ",
        ('f', 'e') => "フェ",
        ('f', 'o') => "フォ",
        ('b' | 'v', 'a') => "バ",
        ('b' | 'v', 'i') => "ビ",
        ('b' | 'v', 'u') => "ブ",
        ('b' | 'v', 'e') => "ベ",
        ('b' | 'v', 'o') => "ボ",
        ('p', 'a') => "パ",
        ('p', 'i') => "ピ",
        ('p', 'u') => "プ",
        ('p', 'e') => "ペ",
        ('p', 'o') => "ポ",
        ('m', 'a') => "マ",
        ('m', 'i') => "ミ",
        ('m', 'u') => "ム",
        ('m', 'e') => "メ",
        ('m', 'o') => "モ",
        ('y', 'a') => "ヤ",
        ('y', 'u') => "ユ",
        ('y', 'e') => "イェ",
        ('y', 'o') => "ヨ",
        ('r' | 'l', 'a') => "ラ",
        ('r' | 'l', 'i') => "リ",
        ('r' | 'l', 'u') => "ル",
        ('r' | 'l', 'e') => "レ",
        ('r' | 'l', 'o') => "ロ",
        ('w', 'a') => "ワ",
        ('w', 'i') => "ウィ",
        ('w', 'u') => "ウ",
        ('w', 'e') => "ウェ",
        ('w', 'o') => "ウォ",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acronyms_are_spelled_without_breaking_uppercase_words() {
        assert_eq!(foreign_word_to_katakana("NVDA"), "エヌブイディーエー");
        assert_eq!(foreign_word_to_katakana("PDF"), "ピーディーエフ");
        assert_eq!(foreign_word_to_katakana("Ctrl"), "シーティーアールエル");
        assert!(!is_spelled_acronym("PAUSE"));
        assert!(!is_spelled_acronym("STOP"));
        assert!(!is_spelled_acronym("ERROR"));
    }

    #[test]
    fn ipa_to_katakana_converts_english_loanwords_cleanly() {
        assert_eq!(ipa_to_katakana("pˈɔːz."), "ポーズ");
        assert_eq!(ipa_to_katakana("dʒˈɛnəɹˌeɪtɚ."), "ジェナレイター");
        assert_eq!(ipa_to_katakana("kˈiːbɔːɹd."), "キーボード");
        assert_eq!(ipa_to_katakana("wˈɜːld."), "ワールド");
        assert_eq!(ipa_to_katakana("kəmpjˈuːtɚ."), "カンピューター");
        assert_eq!(ipa_to_katakana("skɹˈiːn."), "スクリーン");
        assert_eq!(ipa_to_katakana("stˈɑp."), "ストップ");
        assert_eq!(ipa_to_katakana("tʃˈɛk."), "チェック");
        assert_eq!(ipa_to_katakana("vˈɔɪs."), "ボイス");
        assert_eq!(ipa_to_katakana("blˈæŋkoʊ."), "ブランコー");
    }

    #[test]
    fn romaji_fallback_converts_latin_and_japanese_words() {
        assert_eq!(romaji_to_katakana("blanco"), "ブランコ");
        assert_eq!(romaji_to_katakana("konnichiwa"), "コンニチワ");
        assert_eq!(ipa_to_katakana("dʒˈɛnɚɹˌeɪɾɚ."), "ジェナレイター");
        assert_eq!(ipa_to_katakana("wˈɔːɾɚ."), "ウォーター");
        assert_eq!(romaji_to_katakana("apple"), "アップレ");
    }

    #[cfg(feature = "espeak")]
    #[test]
    fn espeak_en_us_lexical_sets_leave_no_unmapped_ipa_chars() {
        let probe = "kit";
        if dengjen_espeak_phonemizer::text_to_phonemes(probe, "en-us", None, true, false).is_err()
        {
            return;
        }
        let sample_words = [
            "kit", "dress", "trap", "lot", "strut", "foot", "bath", "cloth", "nurse", "bird",
            "word", "fleece", "palm", "thought", "goose", "start", "north", "force", "more",
            "door", "cure", "near", "square", "price", "mouth", "choice", "face", "goat",
            "happy", "letter", "generator", "comma", "power", "hour", "fire", "bottle", "button",
            "rhythm", "measure", "church", "judge", "think", "this", "sing", "quick", "switch",
            "computer", "music", "queue", "box", "blanco", "check", "update", "download",
        ];
        let mut failures = Vec::new();
        for word in sample_words {
            let phonemes = dengjen_espeak_phonemizer::text_to_phonemes(
                word, "en-us", None, true, false,
            )
            .unwrap()
            .concat();
            let mut unknown = Vec::new();
            parse_ipa(&phonemes, |ch| unknown.push(ch));
            if !unknown.is_empty() {
                failures.push(format!("{word}: ipa=`{phonemes}` unknown={unknown:?}"));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
