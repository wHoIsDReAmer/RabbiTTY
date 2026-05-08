#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
}

include!(concat!(env!("OUT_DIR"), "/translations.rs"));

use std::sync::{OnceLock, RwLock};

#[derive(Debug, Clone, Copy)]
pub struct LocaleInfo {
    pub tag: &'static str,
    pub native_label: &'static str,
}

pub fn is_known_locale(tag: &str) -> bool {
    AVAILABLE_LOCALES.iter().any(|l| l.tag == tag)
}

static CURRENT_LOCALE: RwLock<&'static str> = RwLock::new("en");
static INITIALIZED: OnceLock<()> = OnceLock::new();

fn fallback_locale() -> &'static str {
    AVAILABLE_LOCALES.first().map(|l| l.tag).unwrap_or("en")
}

fn detect_locale_from_env() -> &'static str {
    let lang = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .unwrap_or_default()
        .to_lowercase();

    AVAILABLE_LOCALES
        .iter()
        .find(|l| lang.starts_with(l.tag))
        .map(|l| l.tag)
        .unwrap_or_else(fallback_locale)
}

fn resolve(locale: Option<&str>) -> &'static str {
    let candidate = locale.map(str::trim).unwrap_or("");
    if candidate.is_empty() || candidate == "auto" {
        return detect_locale_from_env();
    }
    AVAILABLE_LOCALES
        .iter()
        .find(|l| l.tag == candidate)
        .map(|l| l.tag)
        .unwrap_or_else(detect_locale_from_env)
}

pub fn set_locale(locale: Option<&str>) {
    let resolved = resolve(locale);
    *CURRENT_LOCALE.write().expect("i18n locale lock poisoned") = resolved;
    let _ = INITIALIZED.set(());
}

pub fn t(key: &'static str) -> &'static str {
    if INITIALIZED.get().is_none() {
        set_locale(None);
    }
    let locale = *CURRENT_LOCALE.read().expect("i18n locale lock poisoned");
    get_translation(locale, key).unwrap_or(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_translation_works() {
        assert_eq!(
            get_translation("en", "shell_picker.title"),
            Some("Start New Session")
        );
    }

    #[test]
    fn ko_translation_works() {
        assert_eq!(
            get_translation("ko", "shell_picker.title"),
            Some("새 세션 시작")
        );
    }

    #[test]
    fn unknown_key_returns_none() {
        assert_eq!(get_translation("en", "nonexistent.key"), None);
    }

    #[test]
    fn set_locale_switches_active_translation() {
        set_locale(Some("ko"));
        assert_eq!(t("shell_picker.title"), "새 세션 시작");
        set_locale(Some("en"));
        assert_eq!(t("shell_picker.title"), "Start New Session");
    }
}
