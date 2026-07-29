use super::defaults::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShortcutId {
    NewTab,
    CloseTab,
    OpenSettings,
    CommandPalette,
    NextTab,
    PrevTab,
    Quit,
    FontSizeIncrease,
    FontSizeDecrease,
    FontSizeReset,
    DuplicateTab,
    SplitAuto,
    SplitRight,
    SplitDown,
    ClosePane,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
}

impl ShortcutId {
    pub const ALL: [Self; 19] = [
        Self::NewTab,
        Self::CloseTab,
        Self::DuplicateTab,
        Self::NextTab,
        Self::PrevTab,
        Self::SplitAuto,
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
        Self::CommandPalette,
        Self::OpenSettings,
        Self::Quit,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::NewTab => "new_tab",
            Self::CloseTab => "close_tab",
            Self::OpenSettings => "open_settings",
            Self::CommandPalette => "command_palette",
            Self::NextTab => "next_tab",
            Self::PrevTab => "prev_tab",
            Self::Quit => "quit",
            Self::FontSizeIncrease => "font_size_increase",
            Self::FontSizeDecrease => "font_size_decrease",
            Self::FontSizeReset => "font_size_reset",
            Self::DuplicateTab => "duplicate_tab",
            Self::SplitAuto => "split_auto",
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
            Self::CommandPalette => crate::t!("settings.shortcuts.command_palette"),
            Self::NextTab => crate::t!("settings.shortcuts.next_tab"),
            Self::PrevTab => crate::t!("settings.shortcuts.prev_tab"),
            Self::Quit => crate::t!("settings.shortcuts.quit"),
            Self::FontSizeIncrease => crate::t!("settings.shortcuts.font_size_increase"),
            Self::FontSizeDecrease => crate::t!("settings.shortcuts.font_size_decrease"),
            Self::FontSizeReset => crate::t!("settings.shortcuts.font_size_reset"),
            Self::DuplicateTab => crate::t!("settings.shortcuts.duplicate_tab"),
            Self::SplitAuto => crate::t!("settings.shortcuts.split_auto"),
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
            Self::CommandPalette => DEFAULT_SHORTCUT_COMMAND_PALETTE,
            Self::NextTab => DEFAULT_SHORTCUT_NEXT_TAB,
            Self::PrevTab => DEFAULT_SHORTCUT_PREV_TAB,
            Self::Quit => DEFAULT_SHORTCUT_QUIT,
            Self::FontSizeIncrease => DEFAULT_SHORTCUT_FONT_SIZE_INCREASE,
            Self::FontSizeDecrease => DEFAULT_SHORTCUT_FONT_SIZE_DECREASE,
            Self::FontSizeReset => DEFAULT_SHORTCUT_FONT_SIZE_RESET,
            Self::DuplicateTab => DEFAULT_SHORTCUT_DUPLICATE_TAB,
            Self::SplitAuto => DEFAULT_SHORTCUT_SPLIT_AUTO,
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

pub const PLUGIN_PREFIX: &str = "plugin:";

#[derive(Debug, Clone)]
pub struct ShortcutsConfig {
    bindings: BTreeMap<ShortcutId, String>,
    plugin_bindings: BTreeMap<String, String>,
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            bindings: ShortcutId::ALL
                .into_iter()
                .map(|id| (id, id.default_binding().to_string()))
                .collect(),
            plugin_bindings: BTreeMap::new(),
        }
    }
}

pub fn plugin_key(plugin: &str, command: &str) -> String {
    format!("{PLUGIN_PREFIX}{plugin}/{command}")
}

pub fn split_plugin_key(key: &str) -> Option<(&str, &str)> {
    key.strip_prefix(PLUGIN_PREFIX)?.split_once('/')
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

    pub fn plugin_binding(&self, plugin: &str, command: &str) -> Option<&str> {
        self.plugin_bindings
            .get(&plugin_key(plugin, command))
            .map(String::as_str)
    }

    pub fn set_plugin_binding(&mut self, plugin: &str, command: &str, binding: String) {
        let key = plugin_key(plugin, command);
        if binding.trim().is_empty() {
            self.plugin_bindings.remove(&key);
        } else {
            self.plugin_bindings.insert(key, binding);
        }
    }

    pub fn plugin_iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.plugin_bindings
            .iter()
            .map(|(key, binding)| (key.as_str(), binding.as_str()))
    }

    pub fn is_taken(&self, binding: &str) -> bool {
        let binding = binding.trim();
        if binding.is_empty() {
            return false;
        }
        self.bindings
            .values()
            .chain(self.plugin_bindings.values())
            .any(|existing| existing.eq_ignore_ascii_case(binding))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plugin_binding_round_trips_through_its_key() {
        let mut config = ShortcutsConfig::default();
        config.set_plugin_binding("hello", "hello.hi", "Ctrl+Shift+H".to_string());

        assert_eq!(
            config.plugin_binding("hello", "hello.hi"),
            Some("Ctrl+Shift+H")
        );
        let (plugin, command) = config
            .plugin_iter()
            .next()
            .and_then(|(key, _)| split_plugin_key(key))
            .expect("a plugin key");
        assert_eq!((plugin, command), ("hello", "hello.hi"));
    }

    #[test]
    fn a_plugin_key_never_collides_with_a_builtin_key() {
        for id in ShortcutId::ALL {
            assert!(
                split_plugin_key(id.key()).is_none(),
                "{} would be read back as a plugin binding",
                id.key()
            );
        }
    }

    #[test]
    fn clearing_a_plugin_binding_removes_it() {
        let mut config = ShortcutsConfig::default();
        config.set_plugin_binding("hello", "hello.hi", "Ctrl+Shift+H".to_string());
        config.set_plugin_binding("hello", "hello.hi", "  ".to_string());

        assert_eq!(config.plugin_binding("hello", "hello.hi"), None);
        assert_eq!(config.plugin_iter().count(), 0);
    }

    #[test]
    fn a_builtin_binding_counts_as_taken() {
        let config = ShortcutsConfig::default();

        assert!(
            config.is_taken(ShortcutId::Quit.default_binding()),
            "a plugin must not be able to claim the quit key"
        );
        assert!(config.is_taken(&ShortcutId::Quit.default_binding().to_lowercase()));
        assert!(!config.is_taken("Ctrl+Shift+F19"));
        assert!(!config.is_taken("   "));
    }

    #[test]
    fn a_plugin_binding_also_counts_as_taken() {
        let mut config = ShortcutsConfig::default();
        config.set_plugin_binding("hello", "hello.hi", "Ctrl+Shift+H".to_string());

        assert!(
            config.is_taken("Ctrl+Shift+H"),
            "two plugins must not share a key"
        );
    }

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
