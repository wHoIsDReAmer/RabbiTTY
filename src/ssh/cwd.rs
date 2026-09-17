pub(super) fn parse_osc7_cwd(bytes: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"\x1b]7;";
    let mut latest = None;
    let mut base = 0;
    while base < bytes.len() {
        let Some(rel) = find_subslice(&bytes[base..], PREFIX) else {
            break;
        };
        let payload_start = base + rel + PREFIX.len();
        let Some(term) = find_osc_terminator(&bytes[payload_start..]) else {
            break; // incomplete; wait for more data
        };

        if let Some(path) = decode_osc7_payload(&bytes[payload_start..payload_start + term]) {
            latest = Some(path);
        }
        base = payload_start + term + 1;
    }
    latest
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Offset of the OSC terminator (BEL, or ESC `\`) within `bytes`.
fn find_osc_terminator(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .position(|&b| b == 0x07)
        .or_else(|| bytes.windows(2).position(|w| w == [0x1b, b'\\']))
}

/// Decodes an OSC 7 payload (`file://host/path`) into the absolute path.
fn decode_osc7_payload(payload: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(payload).ok()?;
    let rest = text.strip_prefix("file://")?;
    let slash = rest.find('/')?;
    Some(percent_decode(&rest[slash..]))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2]))
        {
            out.push(hi * 16 + lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Wraps a string in single quotes for safe use as one shell word.
pub(super) fn shell_single_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn osc7_parses_path_with_bel_and_st_terminators() {
        let bel = b"prompt\x1b]7;file://host/home/user/proj\x07$ ";
        assert_eq!(parse_osc7_cwd(bel), Some("/home/user/proj".to_string()));

        let st = b"\x1b]7;file://host/var/log\x1b\\";
        assert_eq!(parse_osc7_cwd(st), Some("/var/log".to_string()));
    }

    #[test]
    fn osc7_percent_decodes_and_keeps_latest() {
        let bytes = b"\x1b]7;file://h/tmp\x07\x1b]7;file://h/a%20b/c\x07";
        assert_eq!(parse_osc7_cwd(bytes), Some("/a b/c".to_string()));
    }

    #[test]
    fn osc7_ignores_absent_or_incomplete_sequences() {
        assert_eq!(parse_osc7_cwd(b"no escape here"), None);
        assert_eq!(parse_osc7_cwd(b"\x1b]7;file://host/partial"), None);
    }

    #[test]
    fn shell_single_quote_escapes_quotes() {
        assert_eq!(shell_single_quote("/a/b"), "'/a/b'");
        assert_eq!(shell_single_quote("/it's/here"), "'/it'\\''s/here'");
    }
}
