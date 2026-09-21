use std::collections::HashMap;

use base64::Engine as _;

const MAX_PAYLOAD: usize = 4 * 1024;

#[derive(Debug, Default)]
pub struct OscScanner {
    escape: Escape,
    payload: Vec<u8>,
    overflow: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Escape {
    #[default]
    None,
    Esc,
    Csi,
    Osc,
    OscEsc,
    Str,
    StrEsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Text,
    Skipped,
    Osc,
}

impl OscScanner {
    pub fn step(&mut self, byte: u8) -> Step {
        match self.escape {
            Escape::None => match byte {
                0x1b => {
                    self.escape = Escape::Esc;
                    Step::Skipped
                }
                0x00..=0x08 | 0x0b..=0x0c | 0x0e..=0x1f | 0x7f => Step::Skipped,
                _ => Step::Text,
            },
            Escape::Esc => {
                self.escape = match byte {
                    b'[' => Escape::Csi,
                    b']' => {
                        self.payload.clear();
                        self.overflow = false;
                        Escape::Osc
                    }
                    b'P' | b'_' | b'^' | b'X' => Escape::Str,
                    _ => Escape::None,
                };
                Step::Skipped
            }
            Escape::Csi => {
                if (0x40..=0x7e).contains(&byte) {
                    self.escape = Escape::None;
                }
                Step::Skipped
            }
            Escape::Osc => match byte {
                0x07 => {
                    self.escape = Escape::None;
                    self.finished()
                }
                0x1b => {
                    self.escape = Escape::OscEsc;
                    Step::Skipped
                }
                _ => {
                    if self.payload.len() < MAX_PAYLOAD {
                        self.payload.push(byte);
                    } else {
                        self.overflow = true;
                    }
                    Step::Skipped
                }
            },
            Escape::OscEsc => {
                if byte == b'\\' {
                    self.escape = Escape::None;
                    self.finished()
                } else {
                    self.escape = Escape::Esc;
                    let done = self.finished();
                    self.step(byte);
                    done
                }
            }
            Escape::Str => {
                if byte == 0x1b {
                    self.escape = Escape::StrEsc;
                }
                Step::Skipped
            }
            Escape::StrEsc => {
                self.escape = if byte == b'\\' {
                    Escape::None
                } else {
                    Escape::Str
                };
                Step::Skipped
            }
        }
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    fn finished(&self) -> Step {
        if self.overflow {
            Step::Skipped
        } else {
            Step::Osc
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc {
    Cwd(String),
    Mark(Mark),
    Notification(Notification),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    PromptStart,
    InputStart,
    OutputStart,
    Finished(Option<u32>),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notification {
    pub title: Option<String>,
    pub body: String,
}

#[derive(Debug, Default)]
pub struct OscDecoder {
    kitty: HashMap<String, Notification>,
}

impl OscDecoder {
    pub fn decode(&mut self, payload: &[u8]) -> Option<Osc> {
        let text = std::str::from_utf8(payload).ok()?;
        let (number, rest) = text.split_once(';').unwrap_or((text, ""));
        match number {
            "7" => cwd(rest).map(Osc::Cwd),
            "133" => mark(rest).map(Osc::Mark),
            "9" => osc9(rest).map(Osc::Notification),
            "777" => osc777(rest).map(Osc::Notification),
            "99" => self.osc99(rest).map(Osc::Notification),
            _ => None,
        }
    }

    fn osc99(&mut self, rest: &str) -> Option<Notification> {
        let (meta, payload) = rest.split_once(';').unwrap_or((rest, ""));
        let mut id = "0";
        let mut done = true;
        let mut is_title = false;
        let mut encoded = false;
        for field in meta.split(':') {
            match field.split_once('=') {
                Some(("i", value)) => id = value,
                Some(("d", value)) => done = value != "0",
                Some(("p", value)) => {
                    if value == "?" {
                        return None;
                    }
                    is_title = value == "title";
                }
                Some(("e", value)) => encoded = value == "1",
                _ => {}
            }
        }
        let text = if encoded {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(payload)
                .ok()?;
            String::from_utf8_lossy(&bytes).into_owned()
        } else {
            payload.to_string()
        };

        let pending = self.kitty.entry(id.to_string()).or_default();
        if is_title {
            pending
                .title
                .get_or_insert_with(String::new)
                .push_str(&text);
        } else {
            pending.body.push_str(&text);
        }
        if !done {
            return None;
        }
        let notification = self.kitty.remove(id)?;
        (!notification.body.is_empty() || notification.title.is_some()).then_some(notification)
    }
}

fn mark(rest: &str) -> Option<Mark> {
    let (kind, args) = rest.split_once(';').unwrap_or((rest, ""));
    match kind {
        "A" => Some(Mark::PromptStart),
        "B" => Some(Mark::InputStart),
        "C" => Some(Mark::OutputStart),
        "D" => {
            let code = args.split(';').next().unwrap_or("");
            Some(Mark::Finished(code.parse().ok()))
        }
        _ => None,
    }
}

fn osc9(rest: &str) -> Option<Notification> {
    if rest.is_empty() || rest.starts_with("4;") {
        return None;
    }
    Some(Notification {
        title: None,
        body: rest.to_string(),
    })
}

fn osc777(rest: &str) -> Option<Notification> {
    let (kind, rest) = rest.split_once(';')?;
    if kind != "notify" {
        return None;
    }
    match rest.split_once(';') {
        Some((title, body)) => Some(Notification {
            title: Some(title.to_string()),
            body: body.to_string(),
        }),
        None if rest.is_empty() => None,
        None => Some(Notification {
            title: None,
            body: rest.to_string(),
        }),
    }
}

fn cwd(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let path = &rest[rest.find('/')?..];
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

    fn decode_all(chunks: &[&[u8]]) -> Vec<Osc> {
        let mut scanner = OscScanner::default();
        let mut decoder = OscDecoder::default();
        let mut out = Vec::new();
        for chunk in chunks {
            for &byte in *chunk {
                if scanner.step(byte) == Step::Osc
                    && let Some(osc) = decoder.decode(scanner.payload())
                {
                    out.push(osc);
                }
            }
        }
        out
    }

    fn one(bytes: &[u8]) -> Option<Osc> {
        decode_all(&[bytes]).pop()
    }

    #[test]
    fn osc_7_reports_the_working_directory_with_either_terminator() {
        assert_eq!(
            one(b"\x1b]7;file://host/Users/me/src\x07"),
            Some(Osc::Cwd("/Users/me/src".into()))
        );
        assert_eq!(
            one(b"\x1b]7;file://host/tmp\x1b\\"),
            Some(Osc::Cwd("/tmp".into()))
        );
    }

    #[test]
    fn a_percent_escaped_utf8_path_is_decoded() {
        assert_eq!(
            one(b"\x1b]7;file://host/Users/me/%ED%94%84%EB%A1%9C%EC%A0%9D%ED%8A%B8\x07"),
            Some(Osc::Cwd("/Users/me/프로젝트".into()))
        );
    }

    #[test]
    fn a_malformed_osc_7_is_ignored() {
        assert_eq!(one(b"\x1b]7;not-a-uri\x07"), None);
        assert_eq!(one(b"\x1b]7;file://host\x07"), None);
        assert_eq!(one(b"\x1b]7;file://host/bad%zz\x07"), None);
    }

    #[test]
    fn a_sequence_arriving_across_chunks_is_still_found() {
        assert_eq!(
            decode_all(&[b"\x1b]7;file://ho", b"st/var/log\x07"]),
            vec![Osc::Cwd("/var/log".into())]
        );
    }

    #[test]
    fn shell_marks_parse_with_and_without_arguments() {
        assert_eq!(one(b"\x1b]133;A\x07"), Some(Osc::Mark(Mark::PromptStart)));
        assert_eq!(
            one(b"\x1b]133;A;cl=m;aid=12\x07"),
            Some(Osc::Mark(Mark::PromptStart))
        );
        assert_eq!(one(b"\x1b]133;B\x07"), Some(Osc::Mark(Mark::InputStart)));
        assert_eq!(one(b"\x1b]133;C\x07"), Some(Osc::Mark(Mark::OutputStart)));
        assert_eq!(
            one(b"\x1b]133;D;130\x07"),
            Some(Osc::Mark(Mark::Finished(Some(130))))
        );
        assert_eq!(
            one(b"\x1b]133;D\x07"),
            Some(Osc::Mark(Mark::Finished(None)))
        );
        assert_eq!(one(b"\x1b]133;P;k=i\x07"), None);
    }

    #[test]
    fn osc_9_is_a_notification_but_its_progress_form_is_not() {
        assert_eq!(
            one(b"\x1b]9;build finished\x07"),
            Some(Osc::Notification(Notification {
                title: None,
                body: "build finished".into(),
            }))
        );
        assert_eq!(one(b"\x1b]9;4;1;50\x07"), None);
        assert_eq!(one(b"\x1b]9;\x07"), None);
    }

    #[test]
    fn osc_777_carries_a_title_and_a_body() {
        assert_eq!(
            one(b"\x1b]777;notify;Deploy;done in 3s\x07"),
            Some(Osc::Notification(Notification {
                title: Some("Deploy".into()),
                body: "done in 3s".into(),
            }))
        );
        assert_eq!(one(b"\x1b]777;other;x\x07"), None);
    }

    #[test]
    fn osc_99_assembles_a_title_and_body_sent_separately() {
        let got = decode_all(&[
            b"\x1b]99;i=7:d=0:p=title;Tests\x1b\\",
            b"\x1b]99;i=7:d=1:p=body;3 failed\x1b\\",
        ]);
        assert_eq!(
            got,
            vec![Osc::Notification(Notification {
                title: Some("Tests".into()),
                body: "3 failed".into(),
            })]
        );
    }

    #[test]
    fn osc_99_decodes_a_base64_payload_and_ignores_queries() {
        assert_eq!(
            one(b"\x1b]99;e=1;aGVsbG8=\x1b\\"),
            Some(Osc::Notification(Notification {
                title: None,
                body: "hello".into(),
            }))
        );
        assert_eq!(one(b"\x1b]99;p=?;\x1b\\"), None);
    }

    #[test]
    fn an_oversized_sequence_is_dropped_rather_than_truncated_into_nonsense() {
        let mut bytes = b"\x1b]7;file://host/".to_vec();
        bytes.extend(std::iter::repeat_n(b'a', MAX_PAYLOAD + 10));
        bytes.push(0x07);
        assert_eq!(one(&bytes), None);
    }

    #[test]
    fn an_escape_that_cuts_an_osc_short_still_starts_the_next_sequence() {
        let mut scanner = OscScanner::default();
        let steps: Vec<Step> = b"\x1b]7;x\x1b[mA"
            .iter()
            .map(|&byte| scanner.step(byte))
            .collect();
        assert_eq!(
            steps.last(),
            Some(&Step::Text),
            "the A after the CSI is text"
        );
        assert!(
            !steps[..steps.len() - 1].contains(&Step::Text),
            "nothing inside the OSC or the CSI leaks out as text"
        );
    }

    #[test]
    fn a_dcs_payload_never_leaks_out_as_text() {
        let mut scanner = OscScanner::default();
        let steps: Vec<Step> = b"\x1bPq#0;2;0;0;0#0~~\x1b\\Z"
            .iter()
            .map(|&byte| scanner.step(byte))
            .collect();
        assert_eq!(steps.iter().filter(|s| **s == Step::Text).count(), 1);
    }
}
