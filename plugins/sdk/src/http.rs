use std::fmt;

use crate::{Action, ConnectRequest, ConnectTarget, IoFrame, TcpTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnsupportedScheme(String),
    MissingHost,
    InvalidPort,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnsupportedScheme(scheme) => {
                write!(
                    f,
                    "unsupported scheme {scheme:?}: only plain http is available"
                )
            }
            Error::MissingHost => f.write_str("url has no host"),
            Error::InvalidPort => f.write_str("url has an invalid port"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    method: &'static str,
    host: String,
    port: u16,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    pub fn get(url: &str) -> Result<Request, Error> {
        Request::new("GET", url)
    }

    pub fn post(url: &str) -> Result<Request, Error> {
        Request::new("POST", url)
    }

    pub fn new(method: &'static str, url: &str) -> Result<Request, Error> {
        let (scheme, rest) = url.split_once("://").unwrap_or(("http", url));
        if !scheme.eq_ignore_ascii_case("http") {
            return Err(Error::UnsupportedScheme(scheme.to_string()));
        }
        let rest = rest.split('#').next().unwrap_or("");
        let (authority, path) = match rest.find(['/', '?']) {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if !p.is_empty() => (h, p.parse().map_err(|_| Error::InvalidPort)?),
            Some((h, _)) => (h, 80),
            None => (authority, 80),
        };
        if host.is_empty() {
            return Err(Error::MissingHost);
        }
        let path = if path.starts_with('?') {
            format!("/{path}")
        } else {
            path.to_string()
        };
        Ok(Request {
            method,
            host: host.to_string(),
            port,
            path,
            headers: Vec::new(),
            body: Vec::new(),
        })
    }

    pub fn header(mut self, name: &str, value: &str) -> Request {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn body(mut self, bytes: impl Into<Vec<u8>>) -> Request {
        self.body = bytes.into();
        self
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(128 + self.body.len());
        out.extend_from_slice(self.method.as_bytes());
        out.push(b' ');
        out.extend_from_slice(self.path.as_bytes());
        out.extend_from_slice(b" HTTP/1.1\r\nHost: ");
        out.extend_from_slice(self.host.as_bytes());
        if self.port != 80 {
            out.extend_from_slice(format!(":{}", self.port).as_bytes());
        }
        out.extend_from_slice(b"\r\nConnection: close\r\n");
        if !self.body.is_empty() || self.method != "GET" {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        for (name, value) in &self.headers {
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(b": ");
            out.extend_from_slice(value.as_bytes());
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }

    pub fn start(self, id: u32) -> Vec<Action> {
        let data = self.to_bytes();
        vec![
            Action::Connect(ConnectRequest {
                id,
                target: ConnectTarget::Tcp(TcpTarget {
                    host: self.host,
                    port: self.port,
                }),
            }),
            Action::Send(IoFrame { id, data }),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

enum Framing {
    Length(usize),
    Chunked,
    UntilClose,
}

struct Head {
    status: u16,
    headers: Vec<(String, String)>,
    framing: Framing,
}

pub struct ResponseParser {
    id: u32,
    buf: Vec<u8>,
    head: Option<Head>,
    done: bool,
}

impl ResponseParser {
    pub fn new(id: u32) -> ResponseParser {
        ResponseParser {
            id,
            buf: Vec::new(),
            head: None,
            done: false,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn feed(&mut self, frame: &IoFrame) -> Option<Response> {
        if frame.id != self.id || self.done {
            return None;
        }
        self.buf.extend_from_slice(&frame.data);
        if self.head.is_none() {
            let end = find(&self.buf, b"\r\n\r\n")?;
            let head = parse_head(&self.buf[..end])?;
            self.buf.drain(..end + 4);
            self.head = Some(head);
        }
        let head = self.head.as_ref()?;
        let body = match head.framing {
            Framing::Length(len) if self.buf.len() >= len => self.buf[..len].to_vec(),
            Framing::Chunked => decode_chunked(&self.buf)?,
            _ => return None,
        };
        self.done = true;
        let head = self.head.take()?;
        Some(Response {
            status: head.status,
            headers: head.headers,
            body,
        })
    }

    pub fn finish(self) -> Option<Response> {
        if self.done {
            return None;
        }
        let head = self.head?;
        let body = match head.framing {
            Framing::Length(len) if self.buf.len() >= len => self.buf[..len].to_vec(),
            Framing::Length(_) => return None,
            Framing::Chunked => decode_chunked(&self.buf)?,
            Framing::UntilClose => self.buf,
        };
        Some(Response {
            status: head.status,
            headers: head.headers,
            body,
        })
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn parse_head(raw: &[u8]) -> Option<Head> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut lines = text.split("\r\n");
    let status_line = lines.next()?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next()?;
    if !version.starts_with("HTTP/1.") {
        return None;
    }
    let status = parts.next()?.parse().ok()?;
    let mut headers = Vec::new();
    let mut framing = Framing::UntilClose;
    for line in lines {
        let (name, value) = line.split_once(':')?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("content-length") {
            framing = Framing::Length(value.parse().ok()?);
        } else if name.eq_ignore_ascii_case("transfer-encoding")
            && value.to_ascii_lowercase().contains("chunked")
        {
            framing = Framing::Chunked;
        }
        headers.push((name.trim().to_string(), value.to_string()));
    }
    Some(Head {
        status,
        headers,
        framing,
    })
}

fn decode_chunked(buf: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut pos = 0;
    loop {
        let line_end = pos + find(&buf[pos..], b"\r\n")?;
        let size_text = std::str::from_utf8(&buf[pos..line_end]).ok()?;
        let size_text = size_text.split(';').next()?.trim();
        let size = usize::from_str_radix(size_text, 16).ok()?;
        pos = line_end + 2;
        if size == 0 {
            return Some(out);
        }
        let chunk = buf.get(pos..pos + size)?;
        out.extend_from_slice(chunk);
        pos += size;
        if buf.get(pos..pos + 2)? != b"\r\n" {
            return None;
        }
        pos += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_request_bytes_with_header() {
        let request = Request::get("http://example.com:8080/v1/items?x=1")
            .unwrap()
            .header("Accept", "application/json");
        let actions = request.start(7);
        assert_eq!(
            actions[0],
            Action::Connect(ConnectRequest {
                id: 7,
                target: ConnectTarget::Tcp(TcpTarget {
                    host: "example.com".to_string(),
                    port: 8080,
                }),
            })
        );
        let Action::Send(frame) = &actions[1] else {
            panic!("second action must be send");
        };
        assert_eq!(frame.id, 7);
        assert_eq!(
            frame.data,
            b"GET /v1/items?x=1 HTTP/1.1\r\nHost: example.com:8080\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
        );
    }

    #[test]
    fn https_is_rejected() {
        assert_eq!(
            Request::get("https://example.com/").unwrap_err(),
            Error::UnsupportedScheme("https".to_string())
        );
    }

    #[test]
    fn response_parsed_across_frames_split_mid_header() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello";
        let mut parser = ResponseParser::new(3);
        let first = IoFrame {
            id: 3,
            data: raw[..30].to_vec(),
        };
        assert!(parser.feed(&first).is_none());
        let other = IoFrame {
            id: 4,
            data: b"garbage".to_vec(),
        };
        assert!(parser.feed(&other).is_none());
        let second = IoFrame {
            id: 3,
            data: raw[30..].to_vec(),
        };
        let response = parser.feed(&second).unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.header("content-type"), Some("text/plain"));
        assert_eq!(response.body, b"hello");
    }

    #[test]
    fn body_without_length_ends_at_close() {
        let mut parser = ResponseParser::new(1);
        let frame = IoFrame {
            id: 1,
            data: b"HTTP/1.1 404 Not Found\r\nServer: x\r\n\r\nnot ".to_vec(),
        };
        assert!(parser.feed(&frame).is_none());
        let frame = IoFrame {
            id: 1,
            data: b"here".to_vec(),
        };
        assert!(parser.feed(&frame).is_none());
        let response = parser.finish().unwrap();
        assert_eq!(response.status, 404);
        assert_eq!(response.body, b"not here");
    }

    #[test]
    fn chunked_body_is_decoded() {
        let mut parser = ResponseParser::new(1);
        let frame = IoFrame {
            id: 1,
            data: b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n"
                .to_vec(),
        };
        let response = parser.feed(&frame).unwrap();
        assert_eq!(response.body, b"abcde");
    }
}
