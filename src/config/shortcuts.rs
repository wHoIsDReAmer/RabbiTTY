use super::defaults::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShortcutId {
    NewTab,
    CloseTab,
    OpenSettings,
    NextTab,
    PrevTab,
    Quit,
    FontSizeIncrease,
    FontSizeDecrease,
    FontSizeReset,
    DuplicateTab,
}

impl ShortcutId {
    pub const ALL: [Self; 10] = [
        Self::NewTab,
        Self::CloseTab,
        Self::DuplicateTab,
        Self::NextTab,
        Self::PrevTab,
        Self::FontSizeIncrease,
        Self::FontSizeDecrease,
        Self::FontSizeReset,
        Self::OpenSettings,
        Self::Quit,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::NewTab => "new_tab",
            Self::CloseTab => "close_tab",
            Self::OpenSettings => "open_settings",
            Self::NextTab => "next_tab",
            Self::PrevTab => "prev_tab",
            Self::Quit => "quit",
            Self::FontSizeIncrease => "font_size_increase",
            Self::FontSizeDecrease => "font_size_decrease",
            Self::FontSizeReset => "font_size_reset",
            Self::DuplicateTab => "duplicate_tab",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::NewTab => crate::t!("settings.shortcuts.new_tab"),
            Self::CloseTab => crate::t!("settings.shortcuts.close_tab"),
            Self::OpenSettings => crate::t!("settings.shortcuts.open_settings"),
            Self::NextTab => crate::t!("settings.shortcuts.next_tab"),
            Self::PrevTab => crate::t!("settings.shortcuts.prev_tab"),
            Self::Quit => crate::t!("settings.shortcuts.quit"),
            Self::FontSizeIncrease => crate::t!("settings.shortcuts.font_size_increase"),
            Self::FontSizeDecrease => crate::t!("settings.shortcuts.font_size_decrease"),
            Self::FontSizeReset => crate::t!("settings.shortcuts.font_size_reset"),
            Self::DuplicateTab => crate::t!("settings.shortcuts.duplicate_tab"),
        }
    }

    pub fn default_binding(self) -> &'static str {
        match self {
            Self::NewTab => DEFAULT_SHORTCUT_NEW_TAB,
            Self::CloseTab => DEFAULT_SHORTCUT_CLOSE_TAB,
            Self::OpenSettings => DEFAULT_SHORTCUT_OPEN_SETTINGS,
            Self::NextTab => DEFAULT_SHORTCUT_NEXT_TAB,
            Self::PrevTab => DEFAULT_SHORTCUT_PREV_TAB,
            Self::Quit => DEFAULT_SHORTCUT_QUIT,
            Self::FontSizeIncrease => DEFAULT_SHORTCUT_FONT_SIZE_INCREASE,
            Self::FontSizeDecrease => DEFAULT_SHORTCUT_FONT_SIZE_DECREASE,
            Self::FontSizeReset => DEFAULT_SHORTCUT_FONT_SIZE_RESET,
            Self::DuplicateTab => DEFAULT_SHORTCUT_DUPLICATE_TAB,
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|id| id.key() == key)
    }
}

#[derive(Debug, Clone)]
pub struct ShortcutsConfig {
    bindings: BTreeMap<ShortcutId, String>,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            bindings: ShortcutId::ALL
                .into_iter()
                .map(|id| (id, id.default_binding().to_string()))
                .collect(),
        }
    }
}

impl ShortcutsConfig {
    pub fn get(&self, id: ShortcutId) -> &str {
        self.bindings
            .get(&id)
            .map(String::as_str)
            .unwrap_or_else(|| id.default_binding())
    }

    pub fn set(&mut self, id: ShortcutId, binding: String) {
        self.bindings.insert(id, binding);
    }

    pub fn iter(&self) -> impl Iterator<Item = (ShortcutId, &str)> {
        ShortcutId::ALL.into_iter().map(|id| (id, self.get(id)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_id_has_a_unique_toml_key() {
        let mut keys: Vec<_> = ShortcutId::ALL.iter().map(|id| id.key()).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), before);
    }

    #[test]
    fn default_bindings_do_not_collide() {
        let mut seen: Vec<(&str, ShortcutId)> = Vec::new();
        for id in ShortcutId::ALL {
            let binding = id.default_binding();
            if let Some((_, other)) = seen.iter().find(|(b, _)| *b == binding) {
                panic!("{id:?} and {other:?} both default to {binding}");
            }
            seen.push((binding, id));
        }
    }

    #[test]
    fn keys_round_trip() {
        for id in ShortcutId::ALL {
            assert_eq!(ShortcutId::from_key(id.key()), Some(id));
        }
        assert_eq!(ShortcutId::from_key("nope"), None);
    }

    #[test]
    fn defaults_are_populated_for_every_id() {
        let config = ShortcutsConfig::default();
        for id in ShortcutId::ALL {
            assert_eq!(config.get(id), id.default_binding());
        }
    }
}
