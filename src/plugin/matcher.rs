use std::sync::Arc;
use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};

use super::OutputPattern;

const PATTERN_SIZE_LIMIT: usize = 256 * 1024;
const MAX_MATCHES_PER_WINDOW: u32 = 200;
const WINDOW: Duration = Duration::from_secs(1);

pub fn span_at(regex: &Regex, line: &str, col: usize) -> Option<(usize, usize)> {
    let target: usize = line.chars().take(col).map(char::len_utf8).sum();
    regex
        .find_iter(line)
        .map(|found| (found.start(), found.end()))
        .find(|(start, end)| (*start..*end).contains(&target))
}

pub struct Hit {
    pub pattern: String,
    pub start: u32,
    pub end: u32,
}

pub struct OutputMatcher {
    patterns: Vec<(String, Arc<Regex>, bool)>,
    window_started: Option<Instant>,
    matches_in_window: u32,
    dropped: u64,
    warned: bool,
}

impl OutputMatcher {
    pub fn compile(patterns: &[OutputPattern]) -> (Self, Vec<(String, String)>) {
        let mut compiled = Vec::new();
        let mut rejected = Vec::new();

        for pattern in patterns {
            match RegexBuilder::new(&pattern.regex)
                .size_limit(PATTERN_SIZE_LIMIT)
                .build()
            {
                Ok(regex) => {
                    compiled.push((pattern.id.clone(), Arc::new(regex), pattern.clickable))
                }
                Err(err) => rejected.push((pattern.id.clone(), err.to_string())),
            }
        }

        (
            Self {
                patterns: compiled,
                window_started: None,
                matches_in_window: 0,
                dropped: 0,
                warned: false,
            },
            rejected,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    pub fn clickable(&self) -> impl Iterator<Item = (&str, Arc<Regex>)> {
        self.patterns
            .iter()
            .filter(|(_, _, clickable)| *clickable)
            .map(|(id, regex, _)| (id.as_str(), Arc::clone(regex)))
    }

    pub fn take_throttle_warning(&mut self) -> Option<u64> {
        if self.dropped > 0 && !self.warned {
            self.warned = true;
            return Some(self.dropped);
        }
        None
    }

    pub fn hits(&mut self, line: &str, now: Instant) -> Vec<Hit> {
        if self.patterns.is_empty() {
            return Vec::new();
        }

        let found: Vec<(String, u32, u32)> = self
            .patterns
            .iter()
            .filter_map(|(id, regex, _)| {
                regex
                    .find(line)
                    .map(|m| (id.clone(), m.start() as u32, m.end() as u32))
            })
            .collect();

        let mut hits = Vec::new();
        for (pattern, start, end) in found {
            if !self.admit(now) {
                self.dropped += 1;
                continue;
            }
            hits.push(Hit {
                pattern,
                start,
                end,
            });
        }
        hits
    }

    fn admit(&mut self, now: Instant) -> bool {
        match self.window_started {
            Some(started) if now.duration_since(started) < WINDOW => {
                if self.matches_in_window >= MAX_MATCHES_PER_WINDOW {
                    return false;
                }
                self.matches_in_window += 1;
            }
            _ => {
                self.window_started = Some(now);
                self.matches_in_window = 1;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pattern(id: &str, regex: &str) -> OutputPattern {
        OutputPattern {
            id: id.to_string(),
            regex: regex.to_string(),
            clickable: false,
        }
    }

    fn clickable_pattern(id: &str, regex: &str) -> OutputPattern {
        OutputPattern {
            id: id.to_string(),
            regex: regex.to_string(),
            clickable: true,
        }
    }

    fn issue_regex() -> Regex {
        Regex::new(r"#\d+").expect("regex")
    }

    #[test]
    fn a_span_is_found_only_while_the_column_is_inside_it() {
        let regex = issue_regex();
        let line = "see #42 now";

        assert_eq!(span_at(&regex, line, 4), Some((4, 7)), "on the '#'");
        assert_eq!(span_at(&regex, line, 6), Some((4, 7)), "on the last digit");
        assert_eq!(span_at(&regex, line, 7), None, "just past the end");
        assert_eq!(span_at(&regex, line, 3), None, "just before the start");
    }

    #[test]
    fn columns_are_characters_but_offsets_are_bytes() {
        let regex = issue_regex();
        let line = "안녕 #42 끝";

        assert_eq!(
            span_at(&regex, line, 3),
            Some((7, 10)),
            "three characters in is seven bytes in"
        );
        assert_eq!(span_at(&regex, line, 0), None);
        assert_eq!(
            &line[7..10],
            "#42",
            "the returned offsets must slice the match, not split a character"
        );
    }

    #[test]
    fn the_first_containing_span_wins_when_a_line_repeats() {
        let regex = issue_regex();
        let line = "#1 and #22";

        assert_eq!(span_at(&regex, line, 0), Some((0, 2)));
        assert_eq!(span_at(&regex, line, 8), Some((7, 10)));
    }

    #[test]
    fn only_clickable_patterns_are_offered_for_decoration() {
        let (matcher, rejected) = OutputMatcher::compile(&[
            pattern("plain", "hello"),
            clickable_pattern("issue", r"#\d+"),
        ]);

        assert!(rejected.is_empty());
        let ids: Vec<&str> = matcher.clickable().map(|(id, _)| id).collect();
        assert_eq!(ids, vec!["issue"]);
    }

    #[test]
    fn a_clickable_pattern_still_matches_output() {
        let (mut matcher, _) = OutputMatcher::compile(&[clickable_pattern("issue", r"#\d+")]);
        let hits = matcher.hits("see #42", Instant::now());

        assert_eq!(hits.len(), 1, "clickable must not disable line matching");
        assert_eq!(hits[0].pattern, "issue");
    }

    #[test]
    fn a_rejected_pattern_is_not_offered_for_decoration() {
        let (matcher, rejected) = OutputMatcher::compile(&[clickable_pattern("bad", "(")]);

        assert_eq!(rejected.len(), 1);
        assert_eq!(matcher.clickable().count(), 0);
    }

    #[test]
    fn a_hit_reports_the_pattern_and_its_span() {
        let (mut matcher, rejected) = OutputMatcher::compile(&[pattern("err", r"error\[\w+\]")]);
        assert!(rejected.is_empty());

        let hits = matcher.hits("thread: error[E0308] here", Instant::now());

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].pattern, "err");
        assert_eq!(hits[0].start, 8);
        assert_eq!(hits[0].end, 20);
    }

    #[test]
    fn a_line_can_hit_several_patterns() {
        let (mut matcher, _) =
            OutputMatcher::compile(&[pattern("a", "alpha"), pattern("b", "beta")]);

        let hits = matcher.hits("alpha and beta", Instant::now());

        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn an_uncompilable_pattern_is_rejected_without_losing_the_others() {
        let (matcher, rejected) =
            OutputMatcher::compile(&[pattern("bad", "("), pattern("good", "ok")]);

        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].0, "bad");
        assert!(!matcher.is_empty(), "the valid pattern must survive");
    }

    #[test]
    fn anchors_work_because_the_line_reader_strips_escapes() {
        let (mut matcher, _) = OutputMatcher::compile(&[pattern("c", "^   Compiling")]);

        assert_eq!(
            matcher.hits("   Compiling rabbitty", Instant::now()).len(),
            1
        );
    }

    #[test]
    fn a_flood_is_capped_within_the_window() {
        let (mut matcher, _) = OutputMatcher::compile(&[pattern("all", ".")]);
        let now = Instant::now();

        let mut delivered = 0;
        for _ in 0..(MAX_MATCHES_PER_WINDOW + 50) {
            delivered += matcher.hits("x", now).len() as u32;
        }

        assert_eq!(delivered, MAX_MATCHES_PER_WINDOW);
        assert_eq!(
            matcher.take_throttle_warning(),
            Some(50),
            "the user must be told the plugin is being throttled"
        );
        assert_eq!(
            matcher.take_throttle_warning(),
            None,
            "the warning is reported once, not per dropped match"
        );
    }

    #[test]
    fn the_cap_resets_once_the_window_passes() {
        let (mut matcher, _) = OutputMatcher::compile(&[pattern("all", ".")]);
        let start = Instant::now();

        for _ in 0..(MAX_MATCHES_PER_WINDOW + 10) {
            matcher.hits("x", start);
        }
        assert!(matcher.hits("x", start).is_empty());

        let later = start + WINDOW + Duration::from_millis(1);
        assert_eq!(matcher.hits("x", later).len(), 1);
    }

    #[test]
    fn no_patterns_means_no_work() {
        let (mut matcher, _) = OutputMatcher::compile(&[]);

        assert!(matcher.is_empty());
        assert!(matcher.hits("anything", Instant::now()).is_empty());
    }
}
