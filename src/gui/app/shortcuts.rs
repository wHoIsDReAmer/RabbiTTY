use crate::config::{ShortcutId, ShortcutsConfig};
use iced::keyboard::{Key, Modifiers, key::Named};
use std::borrow::Cow;

fn ascii_char_to_static(ch: char) -> Option<&'static str> {
    static TABLE: [&str; 128] = {
        let mut t = [""; 128];
        // A-Z
        t[b'A' as usize] = "A";
        t[b'B' as usize] = "B";
        t[b'C' as usize] = "C";
        t[b'D' as usize] = "D";
        t[b'E' as usize] = "E";
        t[b'F' as usize] = "F";
        t[b'G' as usize] = "G";
        t[b'H' as usize] = "H";
        t[b'I' as usize] = "I";
        t[b'J' as usize] = "J";
        t[b'K' as usize] = "K";
        t[b'L' as usize] = "L";
        t[b'M' as usize] = "M";
        t[b'N' as usize] = "N";
        t[b'O' as usize] = "O";
        t[b'P' as usize] = "P";
        t[b'Q' as usize] = "Q";
        t[b'R' as usize] = "R";
        t[b'S' as usize] = "S";
        t[b'T' as usize] = "T";
        t[b'U' as usize] = "U";
        t[b'V' as usize] = "V";
        t[b'W' as usize] = "W";
        t[b'X' as usize] = "X";
        t[b'Y' as usize] = "Y";
        t[b'Z' as usize] = "Z";
        // 0-9
        t[b'0' as usize] = "0";
        t[b'1' as usize] = "1";
        t[b'2' as usize] = "2";
        t[b'3' as usize] = "3";
        t[b'4' as usize] = "4";
        t[b'5' as usize] = "5";
        t[b'6' as usize] = "6";
        t[b'7' as usize] = "7";
        t[b'8' as usize] = "8";
        t[b'9' as usize] = "9";
        // Special
        t[b'[' as usize] = "[";
        t[b']' as usize] = "]";
        t[b'/' as usize] = "/";
        t[b';' as usize] = ";";
        t[b'\'' as usize] = "'";
        t[b'-' as usize] = "-";
        t[b'=' as usize] = "=";
        t[b'`' as usize] = "`";
        t
    };
    let i = ch as usize;
    if i < 128 && !TABLE[i].is_empty() {
        Some(TABLE[i])
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ShortcutAction {
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
    SplitAuto,
    SplitRight,
    SplitDown,
    ClosePane,
    FocusPane(crate::gui::pane::Direction),
}

impl ShortcutAction {
    fn from_id(id: ShortcutId) -> Self {
        use crate::gui::pane::Direction;
        match id {
            ShortcutId::NewTab => Self::NewTab,
            ShortcutId::CloseTab => Self::CloseTab,
            ShortcutId::OpenSettings => Self::OpenSettings,
            ShortcutId::NextTab => Self::NextTab,
            ShortcutId::PrevTab => Self::PrevTab,
            ShortcutId::Quit => Self::Quit,
            ShortcutId::FontSizeIncrease => Self::FontSizeIncrease,
            ShortcutId::FontSizeDecrease => Self::FontSizeDecrease,
            ShortcutId::FontSizeReset => Self::FontSizeReset,
            ShortcutId::DuplicateTab => Self::DuplicateTab,
            ShortcutId::SplitAuto => Self::SplitAuto,
            ShortcutId::SplitRight => Self::SplitRight,
            ShortcutId::SplitDown => Self::SplitDown,
            ShortcutId::ClosePane => Self::ClosePane,
            ShortcutId::FocusLeft => Self::FocusPane(Direction::Left),
            ShortcutId::FocusRight => Self::FocusPane(Direction::Right),
            ShortcutId::FocusUp => Self::FocusPane(Direction::Up),
            ShortcutId::FocusDown => Self::FocusPane(Direction::Down),
        }
    }

    pub(super) fn resolve(
        key: &Key,
        modifiers: Modifiers,
        shortcuts: &ShortcutsConfig,
    ) -> Option<Self> {
        shortcuts
            .iter()
            .find(|(_, binding)| shortcut_matches(binding, key, modifiers))
            .map(|(id, _)| Self::from_id(id))
    }
}

struct ParsedShortcut<'a> {
    modifiers: Modifiers,
    key: Cow<'a, str>,
}

pub(super) fn shortcut_matches(binding: &str, key: &Key, modifiers: Modifiers) -> bool {
    let Some(parsed) = parse_shortcut(binding) else {
        return false;
    };
    let Some(event_key) = event_key_token(key) else {
        return false;
    };

    let tracked = Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT | Modifiers::LOGO;
    let pressed = modifiers & tracked;

    *parsed.key == *event_key && parsed.modifiers == pressed
}

fn parse_shortcut(value: &str) -> Option<ParsedShortcut<'static>> {
    let mut modifiers = Modifiers::default();
    let mut key: Option<Cow<'static, str>> = None;

    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }

        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.insert(Modifiers::CTRL),
            "alt" | "option" => modifiers.insert(Modifiers::ALT),
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "cmd" | "command" | "meta" | "super" => modifiers.insert(Modifiers::COMMAND),
            _ => {
                if key.is_some() {
                    return None;
                }
                key = normalize_shortcut_key_token(token);
                key.as_ref()?;
            }
        }
    }

    Some(ParsedShortcut {
        modifiers,
        key: key?,
    })
}

fn event_key_token(key: &Key) -> Option<Cow<'static, str>> {
    match key {
        Key::Named(named) => match named {
            Named::Enter => Some(Cow::Borrowed("Enter")),
            Named::Tab => Some(Cow::Borrowed("Tab")),
            Named::Space => Some(Cow::Borrowed("Space")),
            Named::Escape => Some(Cow::Borrowed("Escape")),
            Named::ArrowUp => Some(Cow::Borrowed("ArrowUp")),
            Named::ArrowDown => Some(Cow::Borrowed("ArrowDown")),
            Named::ArrowLeft => Some(Cow::Borrowed("ArrowLeft")),
            Named::ArrowRight => Some(Cow::Borrowed("ArrowRight")),
            Named::Home => Some(Cow::Borrowed("Home")),
            Named::End => Some(Cow::Borrowed("End")),
            Named::Delete => Some(Cow::Borrowed("Delete")),
            Named::Backspace => Some(Cow::Borrowed("Backspace")),
            Named::Insert => Some(Cow::Borrowed("Insert")),
            Named::PageUp => Some(Cow::Borrowed("PageUp")),
            Named::PageDown => Some(Cow::Borrowed("PageDown")),
            Named::F1 => Some(Cow::Borrowed("F1")),
            Named::F2 => Some(Cow::Borrowed("F2")),
            Named::F3 => Some(Cow::Borrowed("F3")),
            Named::F4 => Some(Cow::Borrowed("F4")),
            Named::F5 => Some(Cow::Borrowed("F5")),
            Named::F6 => Some(Cow::Borrowed("F6")),
            Named::F7 => Some(Cow::Borrowed("F7")),
            Named::F8 => Some(Cow::Borrowed("F8")),
            Named::F9 => Some(Cow::Borrowed("F9")),
            Named::F10 => Some(Cow::Borrowed("F10")),
            Named::F11 => Some(Cow::Borrowed("F11")),
            Named::F12 => Some(Cow::Borrowed("F12")),
            _ => None,
        },
        Key::Character(c) => {
            let mut chars = c.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }

            if ch.is_ascii_alphabetic() {
                return ascii_char_to_static(ch.to_ascii_uppercase()).map(Cow::Borrowed);
            }

            match ch {
                ',' => Some(Cow::Borrowed("Comma")),
                '.' => Some(Cow::Borrowed("Period")),
                _ => ascii_char_to_static(ch).map(Cow::Borrowed),
            }
        }
        Key::Unidentified => None,
    }
}

fn normalize_shortcut_key_token(value: &str) -> Option<Cow<'static, str>> {
    let lower = value.trim().to_ascii_lowercase();

    let normalized = match lower.as_str() {
        "esc" | "escape" => Cow::Borrowed("Escape"),
        "enter" | "return" => Cow::Borrowed("Enter"),
        "tab" => Cow::Borrowed("Tab"),
        "space" | "spacebar" => Cow::Borrowed("Space"),
        "home" => Cow::Borrowed("Home"),
        "end" => Cow::Borrowed("End"),
        "delete" | "del" => Cow::Borrowed("Delete"),
        "backspace" => Cow::Borrowed("Backspace"),
        "insert" | "ins" => Cow::Borrowed("Insert"),
        "pageup" | "page-up" | "pgup" => Cow::Borrowed("PageUp"),
        "pagedown" | "page-down" | "pgdown" => Cow::Borrowed("PageDown"),
        "up" | "arrowup" => Cow::Borrowed("ArrowUp"),
        "down" | "arrowdown" => Cow::Borrowed("ArrowDown"),
        "left" | "arrowleft" => Cow::Borrowed("ArrowLeft"),
        "right" | "arrowright" => Cow::Borrowed("ArrowRight"),
        "comma" => Cow::Borrowed("Comma"),
        "period" | "dot" => Cow::Borrowed("Period"),
        "f1" => Cow::Borrowed("F1"),
        "f2" => Cow::Borrowed("F2"),
        "f3" => Cow::Borrowed("F3"),
        "f4" => Cow::Borrowed("F4"),
        "f5" => Cow::Borrowed("F5"),
        "f6" => Cow::Borrowed("F6"),
        "f7" => Cow::Borrowed("F7"),
        "f8" => Cow::Borrowed("F8"),
        "f9" => Cow::Borrowed("F9"),
        "f10" => Cow::Borrowed("F10"),
        "f11" => Cow::Borrowed("F11"),
        "f12" => Cow::Borrowed("F12"),
        _ => {
            if lower.chars().count() == 1 {
                let ch = lower.chars().next()?;
                let lookup = if ch.is_ascii_alphanumeric() {
                    ascii_char_to_static(ch.to_ascii_uppercase())
                } else {
                    ascii_char_to_static(ch)
                };
                Cow::Borrowed(lookup?)
            } else {
                return None;
            }
        }
    };

    Some(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_matches_normalized_named_key() {
        let matches = shortcut_matches(
            "ctrl + shift + pgdown",
            &Key::Named(Named::PageDown),
            Modifiers::CTRL | Modifiers::SHIFT,
        );

        assert!(matches);
    }

    #[test]
    fn shortcut_rejects_invalid_binding() {
        let matches =
            shortcut_matches("Ctrl+Unknown", &Key::Character("x".into()), Modifiers::CTRL);

        assert!(!matches);
    }

    #[test]
    fn split_shortcuts_resolve_from_defaults() {
        let shortcuts = crate::config::AppConfig::default().shortcuts;
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::SHIFT
        };
        let key = Key::Character("E".into());

        assert!(matches!(
            ShortcutAction::resolve(&key, modifiers, &shortcuts),
            Some(ShortcutAction::SplitAuto)
        ));
    }

    #[test]
    fn focus_shortcuts_resolve_from_defaults() {
        use crate::gui::pane::Direction;
        let shortcuts = crate::config::AppConfig::default().shortcuts;
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::ALT
        } else {
            Modifiers::CTRL | Modifiers::ALT
        };

        for (named, expected) in [
            (Named::ArrowLeft, Direction::Left),
            (Named::ArrowRight, Direction::Right),
            (Named::ArrowUp, Direction::Up),
            (Named::ArrowDown, Direction::Down),
        ] {
            let action = ShortcutAction::resolve(&Key::Named(named), modifiers, &shortcuts);
            assert!(
                matches!(action, Some(ShortcutAction::FocusPane(d)) if d == expected),
                "{named:?} resolved to {action:?}"
            );
        }
    }

    #[test]
    fn auto_split_shortcut_resolves_from_defaults() {
        let shortcuts = crate::config::AppConfig::default().shortcuts;
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::SHIFT
        };
        let action = ShortcutAction::resolve(&Key::Character("E".into()), modifiers, &shortcuts);
        assert!(
            matches!(action, Some(ShortcutAction::SplitAuto)),
            "resolved to {action:?}"
        );
    }
}
