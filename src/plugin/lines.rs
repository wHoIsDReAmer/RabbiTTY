const MAX_LINE: usize = 8 * 1024;
const MAX_OSC: usize = 4 * 1024;

#[derive(Debug, Default)]
pub struct LineReader {
    buf: Vec<u8>,
    escape: Escape,
    osc: Vec<u8>,
    cwd: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Escape {
    #[default]
    None,
    Esc,
    Csi,
    Osc,
    OscEsc,
}

impl LineReader {
    pub fn feed(&mut self, bytes: &[u8], mut on_line: impl FnMut(&str)) {
        for &byte in bytes {
            if self.consume_escape(byte) {
                continue;
            }
            match byte {
                b'\n' => self.flush(&mut on_line),
                b'\r' => {}
                _ => {
                    self.buf.push(byte);
                    if self.buf.len() >= MAX_LINE {
                        self.flush(&mut on_line);
                    }
                }
            }
        }
    }

    pub fn take_cwd(&mut self) -> Option<String> {
        self.cwd.take()
    }

    fn finish_osc(&mut self) {
        if let Some(path) = cwd_from_osc(&self.osc) {
            self.cwd = Some(path);
        }
        self.osc.clear();
    }

    fn consume_escape(&mut self, byte: u8) -> bool {
        match self.escape {
            Escape::None => match byte {
                0x1b => {
                    self.escape = Escape::Esc;
                    true
                }
                0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1f | 0x7f => true,
                _ => false,
            },
            Escape::Esc => {
                self.escape = match byte {
                    b'[' => Escape::Csi,
                    b']' => {
                        self.osc.clear();
                        Escape::Osc
                    }
                    _ => Escape::None,
                };
                true
            }
            Escape::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.escape = Escape::None;
                }
                true
            }
            Escape::Osc => {
                match byte {
                    0x07 => {
                        self.finish_osc();
                        self.escape = Escape::None;
                    }
                    0x1b => self.escape = Escape::OscEsc,
                    _ => {
                        if self.osc.len() < MAX_OSC {
                            self.osc.push(byte);
                        }
                    }
                }
                true
            }
            Escape::OscEsc => {
                self.finish_osc();
                self.escape = Escape::None;
                true
            }
        }
    }

    fn flush(&mut self, on_line: &mut impl FnMut(&str)) {
        if self.buf.is_empty() {
            return;
        }
        let line = String::from_utf8_lossy(&self.buf);
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            on_line(trimmed);
        }
        self.buf.clear();
    }
}

fn cwd_from_osc(payload: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(payload).ok()?;
    let uri = text.strip_prefix("7;")?;
    let rest = uri.strip_prefix("file://")?;
    let path = match rest.find('/') {
        Some(index) => &rest[index..],
        None => return None,
    };
    let decoded = percent_decode(path)?;
    (!decoded.is_empty()).then_some(decoded)
}

fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let value = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cwd(chunks: &[&[u8]]) -> Option<String> {
        let mut reader = LineReader::default();
        for chunk in chunks {
            reader.feed(chunk, |_| {});
        }
        reader.take_cwd()
    }

    #[test]
    fn osc_7_reports_the_working_directory() {
        assert_eq!(
            cwd(&[b"\x1b]7;file://host/Users/me/src\x07"]),
            Some("/Users/me/src".to_string()),
            "terminated by BEL"
        );
        assert_eq!(
            cwd(&[b"\x1b]7;file://host/tmp\x1b\\"]),
            Some("/tmp".to_string()),
            "terminated by ST"
        );
    }

    #[test]
    fn a_percent_escaped_path_is_decoded() {
        assert_eq!(
            cwd(&[b"\x1b]7;file://host/Users/me/my%20projects"]),
            None,
            "an unterminated sequence reports nothing"
        );
        assert_eq!(
            cwd(&[b"\x1b]7;file://host/Users/me/my%20projects\x07"]),
            Some("/Users/me/my projects".to_string())
        );
    }

    #[test]
    fn a_utf8_path_survives_percent_decoding() {
        assert_eq!(
            cwd(&[b"\x1b]7;file://host/Users/me/%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8\x07"]),
            Some("/Users/me/프로젝트".to_string()),
            "percent bytes must be decoded before they are read as utf-8"
        );
    }

    #[test]
    fn other_osc_sequences_are_not_mistaken_for_a_directory() {
        assert_eq!(cwd(&[b"\x1b]0;some window title\x07"]), None);
        assert_eq!(cwd(&[b"\x1b]777;notify;hi\x07"]), None);
    }

    #[test]
    fn a_malformed_osc_7_is_ignored_rather_than_reported() {
        assert_eq!(cwd(&[b"\x1b]7;not-a-uri\x07"]), None);
        assert_eq!(cwd(&[b"\x1b]7;file://host\x07"]), None, "no path at all");
        assert_eq!(cwd(&[b"\x1b]7;file://host/bad%zz\x07"]), None);
    }

    #[test]
    fn a_directory_arriving_across_chunks_is_still_found() {
        assert_eq!(
            cwd(&[b"\x1b]7;file://ho", b"st/var/log\x07"]),
            Some("/var/log".to_string())
        );
    }

    #[test]
    fn reading_the_directory_consumes_it() {
        let mut reader = LineReader::default();
        reader.feed(b"\x1b]7;file://host/tmp\x07", |_| {});

        assert_eq!(reader.take_cwd(), Some("/tmp".to_string()));
        assert_eq!(reader.take_cwd(), None, "the same change must not repeat");
    }

    #[test]
    fn an_osc_payload_cannot_grow_without_bound() {
        let mut reader = LineReader::default();
        let huge = vec![b'x'; MAX_OSC * 2];
        reader.feed(b"\x1b]7;", |_| {});
        reader.feed(&huge, |_| {});

        assert!(reader.osc.len() <= MAX_OSC);
    }

    fn lines(chunks: &[&[u8]]) -> Vec<String> {
        let mut reader = LineReader::default();
        let mut out = Vec::new();
        for chunk in chunks {
            reader.feed(chunk, |line| out.push(line.to_string()));
        }
        out
    }

    #[test]
    fn splits_on_newlines() {
        assert_eq!(lines(&[b"one\ntwo\n"]), vec!["one", "two"]);
    }

    #[test]
    fn an_unterminated_line_is_withheld_until_its_newline_arrives() {
        assert_eq!(lines(&[b"partial"]), Vec::<String>::new());
        assert_eq!(lines(&[b"par", b"tial\n"]), vec!["partial"]);
    }

    #[test]
    fn colour_codes_are_stripped() {
        assert_eq!(
            lines(&[b"\x1b[31merror\x1b[0m: failed\n"]),
            vec!["error: failed"]
        );
    }

    #[test]
    fn an_anchor_can_match_after_stripping() {
        let out = lines(&[b"\x1b[1;32m   Compiling rabbitty\x1b[0m\n"]);
        assert!(
            out[0].starts_with("   Compiling"),
            "the leading escape must not survive: {out:?}"
        );
    }

    #[test]
    fn osc_sequences_are_stripped() {
        assert_eq!(
            lines(&[b"\x1b]0;window title\x07text\n"]),
            vec!["text"],
            "OSC terminated by BEL"
        );
        assert_eq!(
            lines(&[b"\x1b]7;file:///tmp\x1b\\text\n"]),
            vec!["text"],
            "OSC terminated by ST"
        );
    }

    #[test]
    fn a_carriage_return_does_not_split() {
        assert_eq!(lines(&[b"progress\rdone\n"]), vec!["progressdone"]);
    }

    #[test]
    fn a_line_without_a_newline_is_flushed_at_the_cap() {
        let huge = vec![b'x'; MAX_LINE + 10];
        let out = lines(&[&huge]);

        assert_eq!(out.len(), 1, "the cap must bound the buffer");
        assert_eq!(out[0].len(), MAX_LINE);
    }

    #[test]
    fn blank_lines_are_dropped() {
        assert_eq!(lines(&[b"\n\n  \n"]), Vec::<String>::new());
    }

    #[test]
    fn utf8_survives_chunk_boundaries() {
        assert_eq!(
            lines(&["안녕".as_bytes(), "하세요\n".as_bytes()]),
            vec!["안녕하세요"]
        );
    }
}
