pub(super) fn sanitize_positive(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}

pub(super) fn sanitize_opacity(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        value
    } else {
        fallback
    }
}

pub(super) fn sanitize_shortcut(value: &str, fallback: &str) -> String {
    normalize_shortcut(value).unwrap_or_else(|| fallback.to_string())
}

pub(super) fn sanitize_padding(value: f32) -> f32 {
    if value.is_finite() && value >= 0.0 {
        value.min(100.0)
    } else {
        0.0
    }
}

pub(super) fn sanitize_language(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if crate::i18n::is_known_locale(trimmed) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

pub(super) fn sanitize_terminal_font_selection(value: &str) -> Option<String> {
    let selection = value.trim();
    if selection.is_empty() {
        None
    } else {
        Some(selection.to_string())
    }
}

pub(super) fn sanitize_scrollback(value: usize, fallback: usize) -> usize {
    if (100..=1_000_000).contains(&value) {
        value
    } else {
        fallback
    }
}

pub(super) fn sanitize_scroll_multiplier(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (0.1..=10.0).contains(&value) {
        value
    } else {
        fallback
    }
}

pub(super) fn sanitize_terminal_font_size(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && (6.0..=72.0).contains(&value) {
        value
    } else {
        fallback
    }
}

pub(super) fn normalize_shortcut(value: &str) -> Option<String> {
    let mut has_ctrl = false;
    let mut has_alt = false;
    let mut has_shift = false;
    let mut has_command = false;
    let mut key: Option<String> = None;

    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }

        let normalized = token.to_ascii_lowercase();
        match normalized.as_str() {
            "ctrl" | "control" => has_ctrl = true,
            "alt" | "option" => has_alt = true,
            "shift" => has_shift = true,
            "cmd" | "command" | "meta" | "super" => has_command = true,
            _ => {
                if key.is_some() {
                    return None;
                }
                key = normalize_shortcut_key(token);
                key.as_ref()?;
            }
        }
    }

    let key = key?;
    let mut parts: Vec<String> = Vec::new();
    if has_command {
        parts.push("Command".to_string());
    }
    if has_ctrl {
        parts.push("Ctrl".to_string());
    }
    if has_alt {
        parts.push("Alt".to_string());
    }
    if has_shift {
        parts.push("Shift".to_string());
    }
    parts.push(key);

    Some(parts.join("+"))
}

fn normalize_shortcut_key(value: &str) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();
    let canonical = match lower.as_str() {
        "esc" | "escape" => "Escape",
        "enter" | "return" => "Enter",
        "tab" => "Tab",
        "space" | "spacebar" => "Space",
        "home" => "Home",
        "end" => "End",
        "delete" | "del" => "Delete",
        "backspace" => "Backspace",
        "insert" | "ins" => "Insert",
        "pageup" | "page-up" | "pgup" => "PageUp",
        "pagedown" | "page-down" | "pgdown" => "PageDown",
        "up" | "arrowup" => "ArrowUp",
        "down" | "arrowdown" => "ArrowDown",
        "left" | "arrowleft" => "ArrowLeft",
        "right" | "arrowright" => "ArrowRight",
        "comma" => "Comma",
        "period" | "dot" => "Period",
        "f1" => "F1",
        "f2" => "F2",
        "f3" => "F3",
        "f4" => "F4",
        "f5" => "F5",
        "f6" => "F6",
        "f7" => "F7",
        "f8" => "F8",
        "f9" => "F9",
        "f10" => "F10",
        "f11" => "F11",
        "f12" => "F12",
        _ => {
            if lower.chars().count() == 1 {
                let ch = lower.chars().next()?;
                if ch.is_ascii_alphanumeric() {
                    return Some(ch.to_ascii_uppercase().to_string());
                }
                if matches!(
                    ch,
                    ',' | '.' | '[' | ']' | '/' | ';' | '\'' | '-' | '=' | '`'
                ) {
                    return Some(ch.to_string());
                }
            }
            return None;
        }
    };

    Some(canonical.to_string())
}

pub(crate) fn parse_hex_color(value: &str) -> Option<[u8; 3]> {
    let value = value.trim();
    let value = value.strip_prefix('#').unwrap_or(value);
    let value = value.strip_prefix("0x").unwrap_or(value);
    if value.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&value[0..2], 16).ok()?;
    let g = u8::from_str_radix(&value[2..4], 16).ok()?;
    let b = u8::from_str_radix(&value[4..6], 16).ok()?;
    Some([r, g, b])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_color_accepts_valid_formats() {
        assert_eq!(parse_hex_color("#aBcD09"), Some([0xab, 0xcd, 0x09]));
        assert_eq!(parse_hex_color("0x112233"), Some([0x11, 0x22, 0x33]));
        assert_eq!(parse_hex_color("445566"), Some([0x44, 0x55, 0x66]));
    }

    #[test]
    fn parse_hex_color_rejects_invalid_values() {
        assert_eq!(parse_hex_color("#12345"), None);
        assert_eq!(parse_hex_color("#gg0011"), None);
        assert_eq!(parse_hex_color(""), None);
    }

    #[test]
    fn shortcut_normalization_handles_aliases_and_order() {
        assert_eq!(
            normalize_shortcut("control + shift + page-down"),
            Some("Ctrl+Shift+PageDown".to_string())
        );
        assert_eq!(normalize_shortcut("meta+t"), Some("Command+T".to_string()));
    }

    #[test]
    fn shortcut_normalization_rejects_invalid_tokens() {
        assert_eq!(normalize_shortcut("Ctrl+"), None);
        assert_eq!(normalize_shortcut("Ctrl+Tab+X"), None);
        assert_eq!(normalize_shortcut("Shift+UnknownKey"), None);
    }
}
