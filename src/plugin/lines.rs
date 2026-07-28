const MAX_LINE: usize = 8 * 1024;

#[derive(Debug, Default)]
pub struct LineReader {
    buf: Vec<u8>,
    escape: Escape,
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
                    b']' => Escape::Osc,
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
                    0x07 => self.escape = Escape::None,
                    0x1b => self.escape = Escape::OscEsc,
                    _ => {}
                }
                true
            }
            Escape::OscEsc => {
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

#[cfg(test)]
mod tests {
    use super::*;

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
