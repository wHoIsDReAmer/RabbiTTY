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
    SplitRight,
    SplitDown,
    ClosePane,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
}

impl ShortcutId {
    pub const ALL: [Self; 17] = [
        Self::NewTab,
        Self::CloseTab,
        Self::DuplicateTab,
        Self::NextTab,
        Self::PrevTab,
        Self::SplitRight,
        Self::SplitDown,
        Self::ClosePane,
        Self::FocusLeft,
        Self::FocusRight,
        Self::FocusUp,
        Self::FocusDown,
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
            Self::SplitRight => "split_right",
            Self::SplitDown => "split_down",
            Self::ClosePane => "close_pane",
            Self::FocusLeft => "focus_left",
            Self::FocusRight => "focus_right",
            Self::FocusUp => "focus_up",
            Self::FocusDown => "focus_down",
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
            Self::SplitRight => crate::t!("settings.shortcuts.split_right"),
            Self::SplitDown => crate::t!("settings.shortcuts.split_down"),
            Self::ClosePane => crate::t!("settings.shortcuts.close_pane"),
            Self::FocusLeft => crate::t!("settings.shortcuts.focus_left"),
            Self::FocusRight => crate::t!("settings.shortcuts.focus_right"),
            Self::FocusUp => crate::t!("settings.shortcuts.focus_up"),
            Self::FocusDown => crate::t!("settings.shortcuts.focus_down"),
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
            Self::SplitRight => DEFAULT_SHORTCUT_SPLIT_RIGHT,
            Self::SplitDown => DEFAULT_SHORTCUT_SPLIT_DOWN,
            Self::ClosePane => DEFAULT_SHORTCUT_CLOSE_PANE,
            Self::FocusLeft => DEFAULT_SHORTCUT_FOCUS_LEFT,
            Self::FocusRight => DEFAULT_SHORTCUT_FOCUS_RIGHT,
            Self::FocusUp => DEFAULT_SHORTCUT_FOCUS_UP,
            Self::FocusDown => DEFAULT_SHORTCUT_FOCUS_DOWN,
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
