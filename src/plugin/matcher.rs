use std::time::{Duration, Instant};

use regex::{Regex, RegexBuilder};

use super::OutputPattern;

const PATTERN_SIZE_LIMIT: usize = 256 * 1024;
const MAX_MATCHES_PER_WINDOW: u32 = 200;
const WINDOW: Duration = Duration::from_secs(1);

pub struct Hit {
    pub pattern: String,
    pub start: u32,
    pub end: u32,
}

pub struct OutputMatcher {
    patterns: Vec<(String, Regex)>,
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
                Ok(regex) => compiled.push((pattern.id.clone(), regex)),
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
            .filter_map(|(id, regex)| {
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
        }
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
