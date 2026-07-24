use crate::config::{ShortcutId, ShortcutsConfig};
use iced::keyboard::Modifiers;
use iced::keyboard::key::{Code, Physical};
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
        physical: &Physical,
        modifiers: Modifiers,
        shortcuts: &ShortcutsConfig,
    ) -> Option<Self> {
        shortcuts
            .iter()
            .find(|(_, binding)| shortcut_matches(binding, physical, modifiers))
            .map(|(id, _)| Self::from_id(id))
    }
}

struct ParsedShortcut<'a> {
    modifiers: Modifiers,
    key: Cow<'a, str>,
}

pub(super) fn shortcut_matches(binding: &str, physical: &Physical, modifiers: Modifiers) -> bool {
    let Some(parsed) = parse_shortcut(binding) else {
        return false;
    };
    let Some(event_key) = physical_key_token(physical) else {
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

pub(super) fn physical_key_token(physical: &Physical) -> Option<Cow<'static, str>> {
    let Physical::Code(code) = physical else {
        return None;
    };
    let token = match code {
        Code::KeyA => "A",
        Code::KeyB => "B",
        Code::KeyC => "C",
        Code::KeyD => "D",
        Code::KeyE => "E",
        Code::KeyF => "F",
        Code::KeyG => "G",
        Code::KeyH => "H",
        Code::KeyI => "I",
        Code::KeyJ => "J",
        Code::KeyK => "K",
        Code::KeyL => "L",
        Code::KeyM => "M",
        Code::KeyN => "N",
        Code::KeyO => "O",
        Code::KeyP => "P",
        Code::KeyQ => "Q",
        Code::KeyR => "R",
        Code::KeyS => "S",
        Code::KeyT => "T",
        Code::KeyU => "U",
        Code::KeyV => "V",
        Code::KeyW => "W",
        Code::KeyX => "X",
        Code::KeyY => "Y",
        Code::KeyZ => "Z",
        Code::Digit0 | Code::Numpad0 => "0",
        Code::Digit1 | Code::Numpad1 => "1",
        Code::Digit2 | Code::Numpad2 => "2",
        Code::Digit3 | Code::Numpad3 => "3",
        Code::Digit4 | Code::Numpad4 => "4",
        Code::Digit5 | Code::Numpad5 => "5",
        Code::Digit6 | Code::Numpad6 => "6",
        Code::Digit7 | Code::Numpad7 => "7",
        Code::Digit8 | Code::Numpad8 => "8",
        Code::Digit9 | Code::Numpad9 => "9",
        Code::Enter => "Enter",
        Code::Tab => "Tab",
        Code::Space => "Space",
        Code::Escape => "Escape",
        Code::ArrowUp => "ArrowUp",
        Code::ArrowDown => "ArrowDown",
        Code::ArrowLeft => "ArrowLeft",
        Code::ArrowRight => "ArrowRight",
        Code::Home => "Home",
        Code::End => "End",
        Code::Delete => "Delete",
        Code::Backspace => "Backspace",
        Code::Insert => "Insert",
        Code::PageUp => "PageUp",
        Code::PageDown => "PageDown",
        Code::Comma => "Comma",
        Code::Period => "Period",
        Code::F1 => "F1",
        Code::F2 => "F2",
        Code::F3 => "F3",
        Code::F4 => "F4",
        Code::F5 => "F5",
        Code::F6 => "F6",
        Code::F7 => "F7",
        Code::F8 => "F8",
        Code::F9 => "F9",
        Code::F10 => "F10",
        Code::F11 => "F11",
        Code::F12 => "F12",
        _ => return None,
    };
    Some(Cow::Borrowed(token))
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
            &Physical::Code(Code::PageDown),
            Modifiers::CTRL | Modifiers::SHIFT,
        );

        assert!(matches);
    }

    #[test]
    fn shortcut_rejects_invalid_binding() {
        let matches =
            shortcut_matches("Ctrl+Unknown", &Physical::Code(Code::KeyX), Modifiers::CTRL);

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

        assert!(matches!(
            ShortcutAction::resolve(&Physical::Code(Code::KeyE), modifiers, &shortcuts),
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

        for (code, expected) in [
            (Code::ArrowLeft, Direction::Left),
            (Code::ArrowRight, Direction::Right),
            (Code::ArrowUp, Direction::Up),
            (Code::ArrowDown, Direction::Down),
        ] {
            let action = ShortcutAction::resolve(&Physical::Code(code), modifiers, &shortcuts);
            assert!(
                matches!(action, Some(ShortcutAction::FocusPane(d)) if d == expected),
                "{code:?} resolved to {action:?}"
            );
        }
    }

    #[test]
    fn matching_ignores_ime_composed_logical_key() {
        let shortcuts = crate::config::AppConfig::default().shortcuts;
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::SHIFT
        };

        assert!(matches!(
            ShortcutAction::resolve(&Physical::Code(Code::KeyE), modifiers, &shortcuts),
            Some(ShortcutAction::SplitAuto)
        ));
    }
}
