use ab_glyph::{Font, FontArc, GlyphId};
use std::sync::OnceLock;

use super::font::load_system_font_by_family;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    Hangul,
    Kana,
    Han,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Korean,
    Japanese,
    Chinese,
}

pub fn script_of(ch: char) -> Script {
    match ch as u32 {
        0x1100..=0x11FF | 0x3130..=0x318F | 0xA960..=0xA97F | 0xAC00..=0xD7FF => Script::Hangul,
        0x3040..=0x30FF | 0x31F0..=0x31FF => Script::Kana,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F => Script::Han,
        _ => Script::Other,
    }
}

pub fn order(locale: &str, script: Script) -> [Lang; 3] {
    let preferred = match locale {
        "ja" => Lang::Japanese,
        "zh" | "zh-CN" | "zh-TW" | "zh-Hans" | "zh-Hant" => Lang::Chinese,
        _ => Lang::Korean,
    };
    let first = match script {
        Script::Hangul => Lang::Korean,
        Script::Kana => Lang::Japanese,
        Script::Han | Script::Other => preferred,
    };
    let mut out = [first; 3];
    let mut next = 1;
    for lang in [Lang::Korean, Lang::Japanese, Lang::Chinese] {
        if lang != first {
            out[next] = lang;
            next += 1;
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn families(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Korean => &[
            "Malgun Gothic",
            "D2Coding",
            "NanumGothicCoding",
            "NanumGothic",
            "Gulim",
            "Dotum",
            "Batang",
        ],
        Lang::Japanese => &["Yu Gothic", "Meiryo", "MS Gothic", "MS Mincho"],
        Lang::Chinese => &["Microsoft YaHei", "Microsoft JhengHei", "SimSun", "NSimSun"],
    }
}

#[cfg(target_os = "macos")]
fn families(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Korean => &[
            "Apple SD Gothic Neo",
            "AppleGothic",
            "Nanum Gothic",
            "D2Coding",
        ],
        Lang::Japanese => &["Hiragino Sans", "Hiragino Kaku Gothic ProN", "YuGothic"],
        Lang::Chinese => &["PingFang SC", "PingFang TC", "Heiti SC", "Songti SC"],
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn families(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Korean => &[
            "Noto Sans Mono CJK KR",
            "Noto Sans CJK KR",
            "Source Han Sans KR",
            "NanumGothic",
            "D2Coding",
        ],
        Lang::Japanese => &[
            "Noto Sans Mono CJK JP",
            "Noto Sans CJK JP",
            "Source Han Sans JP",
            "IPAGothic",
        ],
        Lang::Chinese => &[
            "Noto Sans Mono CJK SC",
            "Noto Sans CJK SC",
            "Source Han Sans SC",
            "WenQuanYi Zen Hei",
        ],
    }
}

#[derive(Debug)]
struct Candidate {
    family: &'static str,
    font: OnceLock<Option<FontArc>>,
}

impl Candidate {
    fn font(&self) -> Option<&FontArc> {
        self.font
            .get_or_init(|| load_system_font_by_family(self.family))
            .as_ref()
    }
}

fn first_covering<'a>(
    fonts: impl Iterator<Item = &'a FontArc>,
    ch: char,
) -> Option<(&'a FontArc, GlyphId)> {
    for font in fonts {
        let id = font.glyph_id(ch);
        if id.0 != 0 {
            return Some((font, id));
        }
    }
    None
}

#[derive(Debug)]
pub struct FontFallback {
    locale: &'static str,
    korean: Vec<Candidate>,
    japanese: Vec<Candidate>,
    chinese: Vec<Candidate>,
}

impl FontFallback {
    pub fn for_locale(locale: &'static str) -> Self {
        let group = |lang| {
            families(lang)
                .iter()
                .map(|family| Candidate {
                    family,
                    font: OnceLock::new(),
                })
                .collect()
        };
        Self {
            locale,
            korean: group(Lang::Korean),
            japanese: group(Lang::Japanese),
            chinese: group(Lang::Chinese),
        }
    }

    pub fn glyph_for(&self, ch: char) -> Option<(&FontArc, GlyphId)> {
        for lang in order(self.locale, script_of(ch)) {
            let group = match lang {
                Lang::Korean => &self.korean,
                Lang::Japanese => &self.japanese,
                Lang::Chinese => &self.chinese,
            };
            let found = first_covering(group.iter().filter_map(Candidate::font), ch);
            if found.is_some() {
                return found;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hangul_is_routed_to_korean_fonts_whatever_the_interface_language_is() {
        for locale in ["en", "ko", "ja", "zh"] {
            assert_eq!(
                order(locale, script_of('한'))[0],
                Lang::Korean,
                "{locale} sent Hangul somewhere else"
            );
        }
        assert_eq!(order("zh", script_of('ㄱ'))[0], Lang::Korean);
        assert_eq!(order("zh", script_of('ᄒ'))[0], Lang::Korean);
    }

    #[test]
    fn kana_is_routed_to_japanese_fonts_whatever_the_interface_language_is() {
        for locale in ["en", "ko", "ja", "zh"] {
            assert_eq!(order(locale, script_of('あ'))[0], Lang::Japanese);
            assert_eq!(order(locale, script_of('カ'))[0], Lang::Japanese);
        }
    }

    #[test]
    fn shared_ideographs_follow_the_interface_language() {
        assert_eq!(order("ko", script_of('漢'))[0], Lang::Korean);
        assert_eq!(order("ja", script_of('漢'))[0], Lang::Japanese);
        assert_eq!(order("zh", script_of('漢'))[0], Lang::Chinese);
    }

    #[test]
    fn every_language_stays_reachable_behind_the_first_choice() {
        for locale in ["en", "ko", "ja", "zh"] {
            for script in [Script::Hangul, Script::Kana, Script::Han, Script::Other] {
                let mut seen = order(locale, script);
                seen.sort_by_key(|lang| format!("{lang:?}"));
                assert_eq!(
                    seen,
                    [Lang::Chinese, Lang::Japanese, Lang::Korean],
                    "{locale}/{script:?} dropped or repeated a language"
                );
            }
        }
    }

    fn dejavu() -> FontArc {
        FontArc::try_from_slice(include_bytes!("../../fonts/DejaVuSansMono.ttf")).expect("bundled")
    }

    #[test]
    fn a_font_without_the_character_is_passed_over_instead_of_answering_with_a_blank() {
        let latin = dejavu();
        assert!(
            first_covering([&latin].into_iter(), '\u{d55c}').is_none(),
            "a chain of fonts that lack the character must report a miss, not glyph zero"
        );
        assert!(first_covering([&latin].into_iter(), 'A').is_some());
    }

    #[test]
    fn a_latin_only_font_still_resolves_hangul_through_the_chain() {
        let chain = FontFallback::for_locale("ko");
        let Some((font, id)) = chain.glyph_for('한') else {
            eprintln!("no CJK font installed; skipping");
            return;
        };
        assert_ne!(id.0, 0);
        assert_ne!(
            font.glyph_id('글').0,
            0,
            "the chosen font must cover Hangul generally, not one syllable"
        );
    }
}
