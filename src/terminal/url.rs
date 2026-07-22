const SCHEMES: [&str; 2] = ["https://", "http://"];
const TRAILING: [char; 12] = ['.', ',', ';', ':', '!', '?', '"', '\'', ')', ']', '}', '>'];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlSpan {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

pub fn is_openable(uri: &str) -> bool {
    SCHEMES
        .iter()
        .any(|scheme| uri.len() > scheme.len() && uri[..scheme.len()].eq_ignore_ascii_case(scheme))
}

pub fn url_at(row: &[char], col: usize) -> Option<UrlSpan> {
    if col >= row.len() || row[col].is_whitespace() {
        return None;
    }

    let mut run_start = col;
    while run_start > 0 && !row[run_start - 1].is_whitespace() {
        run_start -= 1;
    }
    let mut run_end = col;
    while run_end + 1 < row.len() && !row[run_end + 1].is_whitespace() {
        run_end += 1;
    }

    let run: String = row[run_start..=run_end].iter().collect();
    let offsets: Vec<usize> = run.char_indices().map(|(i, _)| i).collect();
    let byte_to_cell = |byte: usize| offsets.iter().position(|o| *o == byte);

    let scheme_byte = SCHEMES
        .iter()
        .filter_map(|scheme| run.find(scheme))
        .min_by_key(|byte| *byte)?;
    let start = run_start + byte_to_cell(scheme_byte)?;

    let mut end = run_end;
    while end > start {
        let ch = row[end];
        if !TRAILING.contains(&ch) {
            break;
        }
        if ch == ')' && row[start..=end].contains(&'(') {
            break;
        }
        end -= 1;
    }

    if col < start || col > end {
        return None;
    }

    let url: String = row[start..=end].iter().collect();
    if SCHEMES.iter().any(|scheme| url.len() <= scheme.len()) {
        return None;
    }

    Some(UrlSpan { start, end, url })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn at(text: &str, col: usize) -> Option<String> {
        url_at(&row(text), col).map(|span| span.url)
    }

    #[test]
    fn finds_a_bare_url() {
        let text = "see https://example.com/a here";
        assert_eq!(at(text, 6).as_deref(), Some("https://example.com/a"));
    }

    #[test]
    fn spans_cover_the_whole_url() {
        let text = "x https://a.dev y";
        let span = url_at(&row(text), 5).expect("url");
        assert_eq!(span.start, 2);
        assert_eq!(span.end, 14);
        assert_eq!(span.url, "https://a.dev");
    }

    #[test]
    fn ignores_positions_outside_the_url() {
        let text = "see https://example.com here";
        assert_eq!(at(text, 0), None);
        assert_eq!(at(text, 3), None);
        assert_eq!(at(text, 24), None);
    }

    #[test]
    fn trims_sentence_punctuation() {
        assert_eq!(
            at("go to https://example.com.", 10).as_deref(),
            Some("https://example.com")
        );
        assert_eq!(
            at("(https://example.com)", 5).as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn keeps_a_closing_paren_that_belongs_to_the_url() {
        assert_eq!(
            at("https://en.wikipedia.org/wiki/Rust_(language)", 10).as_deref(),
            Some("https://en.wikipedia.org/wiki/Rust_(language)")
        );
    }

    #[test]
    fn only_http_and_https_are_offered() {
        assert_eq!(at("file:///etc/passwd", 2), None);
        assert_eq!(at("javascript:alert(1)", 2), None);
        assert_eq!(at("ftp://example.com", 2), None);
    }

    #[test]
    fn a_scheme_with_no_host_is_not_a_url() {
        assert_eq!(at("https://", 2), None);
    }

    #[test]
    fn finds_a_url_embedded_in_surrounding_punctuation() {
        assert_eq!(
            at("[link](https://example.com/x)", 15).as_deref(),
            Some("https://example.com/x")
        );
    }

    #[test]
    fn osc8_uris_are_scheme_checked_too() {
        assert!(is_openable("https://example.com"));
        assert!(is_openable("HTTP://EXAMPLE.COM"));
        assert!(!is_openable("file:///etc/passwd"));
        assert!(!is_openable("https://"));
    }

    #[test]
    fn whitespace_is_never_a_url() {
        assert_eq!(at("a https://x.dev b", 1), None);
        assert_eq!(at("", 0), None);
    }
}
