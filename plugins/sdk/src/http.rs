use std::collections::VecDeque;
use std::fmt;
use std::ops::Range;

use crate::{Action, ConnectRequest, ConnectTarget, Event, IoFrame, TcpTarget, Timer};

pub const REQUEST_TIMEOUT_MS: u64 = 60_000;
pub const IDLE_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnsupportedScheme(String),
    MissingHost,
    InvalidPort,
    TlsUnavailable,
    Tls(String),
    Closed(Option<String>),
    Timeout,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::UnsupportedScheme(scheme) => {
                write!(
                    f,
                    "unsupported scheme {scheme:?}: only http and https are available"
                )
            }
            Error::MissingHost => f.write_str("url has no host"),
            Error::InvalidPort => f.write_str("url has an invalid port"),
            Error::TlsUnavailable => f.write_str(
                "https needs TLS: enable the SDK's `tls` feature and call `Client::with_tls`",
            ),
            Error::Tls(reason) => write!(f, "TLS: {reason}"),
            Error::Closed(Some(reason)) => write!(f, "connection closed: {reason}"),
            Error::Closed(None) => f.write_str("connection closed before a full reply"),
            Error::Timeout => f.write_str("no reply in time"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    method: &'static str,
    secure: bool,
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
        let secure = match scheme.to_ascii_lowercase().as_str() {
            "http" => false,
            "https" => true,
            _ => return Err(Error::UnsupportedScheme(scheme.to_string())),
        };
        let default_port = if secure { 443 } else { 80 };
        let rest = rest.split('#').next().unwrap_or("");
        let (authority, path) = match rest.find(['/', '?']) {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if !p.is_empty() => (h, p.parse().map_err(|_| Error::InvalidPort)?),
            Some((h, _)) => (h, default_port),
            None => (authority, default_port),
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
            secure,
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

    pub fn is_secure(&self) -> bool {
        self.secure
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(128 + self.body.len());
        out.extend_from_slice(self.method.as_bytes());
        out.push(b' ');
        out.extend_from_slice(self.path.as_bytes());
        out.extend_from_slice(b" HTTP/1.1\r\nHost: ");
        out.extend_from_slice(self.host.as_bytes());
        if self.port != if self.secure { 443 } else { 80 } {
            out.extend_from_slice(format!(":{}", self.port).as_bytes());
        }
        out.extend_from_slice(b"\r\n");
        if !self.body.is_empty() || self.method != "GET" {
            out.extend_from_slice(format!("Content-Length: {}\r\n", self.body.len()).as_bytes());
        }
        for (name, value) in &self.headers {
            out.extend(name.bytes().filter(|byte| !byte.is_ascii_control()));
            out.extend_from_slice(b": ");
            out.extend(value.bytes().filter(|byte| !byte.is_ascii_control()));
            out.extend_from_slice(b"\r\n");
        }
        out.extend_from_slice(b"\r\n");
        out.extend_from_slice(&self.body);
        out
    }

    fn origin(&self) -> Origin {
        Origin {
            secure: self.secure,
            host: self.host.clone(),
            port: self.port,
        }
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

pub struct Step<T> {
    pub actions: Vec<Action>,
    pub replies: Vec<(T, Result<Response, Error>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    secure: bool,
    host: String,
    port: u16,
}

enum Wire {
    Plain(Vec<u8>),
    #[cfg(feature = "tls")]
    Tls(Box<rustls::ClientConnection>),
}

impl Wire {
    fn send(&mut self, plain: &[u8]) -> Result<(), Error> {
        match self {
            Wire::Plain(pending) => {
                pending.extend_from_slice(plain);
                Ok(())
            }
            #[cfg(feature = "tls")]
            Wire::Tls(conn) => {
                use std::io::Write;
                conn.writer()
                    .write_all(plain)
                    .map_err(|err| Error::Tls(err.to_string()))
            }
        }
    }

    fn outgoing(&mut self) -> Vec<u8> {
        match self {
            Wire::Plain(pending) => std::mem::take(pending),
            #[cfg(feature = "tls")]
            Wire::Tls(conn) => {
                let mut out = Vec::new();
                while conn.wants_write() {
                    match conn.write_tls(&mut out) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                out
            }
        }
    }

    fn receive(&mut self, incoming: &[u8]) -> Result<Vec<u8>, Error> {
        match self {
            Wire::Plain(_) => Ok(incoming.to_vec()),
            #[cfg(feature = "tls")]
            Wire::Tls(conn) => {
                use std::io::Read;
                let tls = |err: &dyn fmt::Display| Error::Tls(err.to_string());
                let mut incoming = incoming;
                let mut plain = Vec::new();
                let mut chunk = [0u8; 16 * 1024];
                while !incoming.is_empty() {
                    let read = conn.read_tls(&mut incoming).map_err(|err| tls(&err))?;
                    conn.process_new_packets().map_err(|err| tls(&err))?;
                    loop {
                        match conn.reader().read(&mut chunk) {
                            Ok(0) => break,
                            Ok(n) => plain.extend_from_slice(&chunk[..n]),
                            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => break,
                            Err(err) => return Err(tls(&err)),
                        }
                    }
                    if read == 0 {
                        break;
                    }
                }
                Ok(plain)
            }
        }
    }
}

enum Link {
    Down,
    Opening {
        id: u32,
        origin: Origin,
        wire: Wire,
    },
    Open {
        id: u32,
        origin: Origin,
        wire: Wire,
        used: bool,
    },
}

struct Exchange<T> {
    tag: T,
    origin: Origin,
    bytes: Vec<u8>,
}

struct Inflight<T> {
    exchange: Exchange<T>,
    parser: ResponseParser,
    reused: bool,
    answered: bool,
}

pub struct Client<T> {
    ids: Range<u32>,
    next_id: u32,
    timer: u32,
    #[cfg(feature = "tls")]
    tls: Option<std::sync::Arc<rustls::ClientConfig>>,
    link: Link,
    queue: VecDeque<Exchange<T>>,
    inflight: Option<Inflight<T>>,
    actions: Vec<Action>,
    replies: Vec<(T, Result<Response, Error>)>,
}

impl<T> Client<T> {
    pub fn new(ids: Range<u32>, timer: u32) -> Self {
        assert!(!ids.is_empty(), "the client needs at least one stream id");
        Self {
            next_id: ids.start,
            ids,
            timer,
            #[cfg(feature = "tls")]
            tls: None,
            link: Link::Down,
            queue: VecDeque::new(),
            inflight: None,
            actions: Vec::new(),
            replies: Vec::new(),
        }
    }

    #[cfg(feature = "tls")]
    pub fn with_tls(
        mut self,
        provider: std::sync::Arc<rustls::crypto::CryptoProvider>,
    ) -> Result<Self, Error> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|err| Error::Tls(err.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        self.tls = Some(std::sync::Arc::new(config));
        Ok(self)
    }

    pub fn busy(&self) -> bool {
        self.inflight.is_some() || !self.queue.is_empty()
    }

    pub fn send(&mut self, tag: T, request: &Request) -> Step<T> {
        self.queue.push_back(Exchange {
            tag,
            origin: request.origin(),
            bytes: request.to_bytes(),
        });
        self.pump();
        self.step()
    }

    pub fn handle(&mut self, event: &Event) -> Option<Step<T>> {
        let current = self.link_id();
        match event {
            Event::Connected(id) if current == Some(*id) => self.connected(),
            Event::Data(frame) if current == Some(frame.id) => self.data(&frame.data),
            Event::Closed(closed) if current == Some(closed.id) => {
                self.closed(closed.reason.clone());
            }
            Event::Timer(id) if *id == self.timer => self.timeout(),
            Event::Connected(id) if self.ids.contains(id) => {}
            Event::Data(frame) if self.ids.contains(&frame.id) => {}
            Event::Closed(closed) if self.ids.contains(&closed.id) => {}
            _ => return None,
        }
        Some(self.step())
    }

    pub fn shutdown(&mut self) -> Vec<Action> {
        self.queue.clear();
        self.inflight = None;
        self.drop_link();
        std::mem::take(&mut self.actions)
    }

    fn step(&mut self) -> Step<T> {
        Step {
            actions: std::mem::take(&mut self.actions),
            replies: std::mem::take(&mut self.replies),
        }
    }

    fn link_id(&self) -> Option<u32> {
        match &self.link {
            Link::Opening { id, .. } | Link::Open { id, .. } => Some(*id),
            Link::Down => None,
        }
    }

    fn connected(&mut self) {
        if let Link::Opening { id, origin, wire } = std::mem::replace(&mut self.link, Link::Down) {
            self.link = Link::Open {
                id,
                origin,
                wire,
                used: false,
            };
            self.pump();
            self.flush();
        }
    }

    fn data(&mut self, incoming: &[u8]) {
        let Link::Open { wire, .. } = &mut self.link else {
            return;
        };
        let plain = match wire.receive(incoming) {
            Ok(plain) => plain,
            Err(err) => return self.fail(err),
        };
        self.flush();
        if plain.is_empty() {
            return;
        }
        let Some(inflight) = self.inflight.as_mut() else {
            return;
        };
        inflight.answered = true;
        let Some(response) = inflight.parser.feed(&IoFrame { id: 0, data: plain }) else {
            return;
        };
        let Some(done) = self.inflight.take() else {
            return;
        };
        let server_closes = response
            .header("connection")
            .is_some_and(|value| value.eq_ignore_ascii_case("close"));
        self.replies.push((done.exchange.tag, Ok(response)));
        if server_closes {
            self.drop_link();
        } else if let Link::Open { used, .. } = &mut self.link {
            *used = true;
        }
        self.pump();
        if self.inflight.is_none() && matches!(self.link, Link::Open { .. }) {
            self.arm(IDLE_TIMEOUT_MS);
        }
    }

    fn closed(&mut self, reason: Option<String>) {
        let was_opening = matches!(self.link, Link::Opening { .. });
        self.link = Link::Down;
        if was_opening {
            return self.fail_queue(&Error::Closed(reason));
        }
        let Some(inflight) = self.inflight.take() else {
            return;
        };
        if let Some(response) = inflight.parser.finish() {
            self.replies.push((inflight.exchange.tag, Ok(response)));
        } else if inflight.reused && !inflight.answered {
            self.queue.push_front(inflight.exchange);
        } else {
            self.replies
                .push((inflight.exchange.tag, Err(Error::Closed(reason))));
        }
        self.pump();
    }

    fn timeout(&mut self) {
        if self.busy() {
            self.fail(Error::Timeout);
        } else {
            self.drop_link();
        }
    }

    fn pump(&mut self) {
        while self.inflight.is_none() {
            let Some(origin) = self.queue.front().map(|exchange| exchange.origin.clone()) else {
                return;
            };
            let current = match &self.link {
                Link::Opening { origin, .. } | Link::Open { origin, .. } => Some(origin),
                Link::Down => None,
            };
            if current.is_some_and(|current| *current != origin) {
                self.drop_link();
            }
            match self.link {
                Link::Opening { .. } => return,
                Link::Open { .. } => return self.dispatch(),
                Link::Down => match self.open(&origin) {
                    Ok(wire) => {
                        let id = self.allocate();
                        self.actions.push(Action::Connect(ConnectRequest {
                            id,
                            target: ConnectTarget::Tcp(TcpTarget {
                                host: origin.host.clone(),
                                port: origin.port,
                            }),
                        }));
                        self.link = Link::Opening { id, origin, wire };
                        return self.arm(REQUEST_TIMEOUT_MS);
                    }
                    Err(err) => {
                        if let Some(exchange) = self.queue.pop_front() {
                            self.replies.push((exchange.tag, Err(err)));
                        }
                    }
                },
            }
        }
    }

    fn dispatch(&mut self) {
        let Some(exchange) = self.queue.pop_front() else {
            return;
        };
        let Link::Open { wire, used, .. } = &mut self.link else {
            self.queue.push_front(exchange);
            return;
        };
        let reused = *used;
        if let Err(err) = wire.send(&exchange.bytes) {
            self.replies.push((exchange.tag, Err(err.clone())));
            return self.fail(err);
        }
        self.inflight = Some(Inflight {
            exchange,
            parser: ResponseParser::new(0),
            reused,
            answered: false,
        });
        self.flush();
        self.arm(REQUEST_TIMEOUT_MS);
    }

    fn open(&self, origin: &Origin) -> Result<Wire, Error> {
        if !origin.secure {
            return Ok(Wire::Plain(Vec::new()));
        }
        #[cfg(feature = "tls")]
        if let Some(config) = &self.tls {
            let name = rustls::pki_types::ServerName::try_from(origin.host.clone())
                .map_err(|err| Error::Tls(err.to_string()))?;
            let conn = rustls::ClientConnection::new(std::sync::Arc::clone(config), name)
                .map_err(|err| Error::Tls(err.to_string()))?;
            return Ok(Wire::Tls(Box::new(conn)));
        }
        Err(Error::TlsUnavailable)
    }

    fn allocate(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = if id + 1 >= self.ids.end {
            self.ids.start
        } else {
            id + 1
        };
        id
    }

    fn arm(&mut self, after_ms: u64) {
        self.actions.push(Action::Schedule(Timer {
            id: self.timer,
            after_ms,
            repeat: false,
        }));
    }

    fn flush(&mut self) {
        if let Link::Open { id, wire, .. } = &mut self.link {
            let data = wire.outgoing();
            if !data.is_empty() {
                self.actions.push(Action::Send(IoFrame { id: *id, data }));
            }
        }
    }

    fn drop_link(&mut self) {
        if let Link::Opening { id, .. } | Link::Open { id, .. } =
            std::mem::replace(&mut self.link, Link::Down)
        {
            self.actions.push(Action::Close(id));
        }
    }

    fn fail(&mut self, err: Error) {
        self.drop_link();
        if let Some(inflight) = self.inflight.take() {
            self.replies.push((inflight.exchange.tag, Err(err.clone())));
        }
        self.fail_queue(&err);
    }

    fn fail_queue(&mut self, err: &Error) {
        for exchange in self.queue.drain(..) {
            self.replies.push((exchange.tag, Err(err.clone())));
        }
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
    use crate::IoClosed;

    const TIMER: u32 = 9;
    const OK: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nhi";

    fn client() -> Client<u32> {
        Client::new(100..110, TIMER)
    }

    fn get(url: &str) -> Request {
        Request::get(url).unwrap()
    }

    fn connects(actions: &[Action]) -> Vec<u32> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Connect(request) => Some(request.id),
                _ => None,
            })
            .collect()
    }

    fn sent(actions: &[Action]) -> Vec<(u32, Vec<u8>)> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Send(frame) => Some((frame.id, frame.data.clone())),
                _ => None,
            })
            .collect()
    }

    fn closes(actions: &[Action]) -> Vec<u32> {
        actions
            .iter()
            .filter_map(|action| match action {
                Action::Close(id) => Some(*id),
                _ => None,
            })
            .collect()
    }

    fn statuses(step: Step<u32>) -> Vec<(u32, Result<u16, Error>)> {
        step.replies
            .into_iter()
            .map(|(tag, reply)| (tag, reply.map(|response| response.status)))
            .collect()
    }

    fn data(id: u32, bytes: &[u8]) -> Event {
        Event::Data(IoFrame {
            id,
            data: bytes.to_vec(),
        })
    }

    fn closed(id: u32, reason: Option<&str>) -> Event {
        Event::Closed(IoClosed {
            id,
            reason: reason.map(str::to_string),
        })
    }

    fn answered_once(client: &mut Client<u32>) -> u32 {
        let id = connects(&client.send(1, &get("http://example.com/")).actions)[0];
        client.handle(&Event::Connected(id)).unwrap();
        let step = client.handle(&data(id, OK)).unwrap();
        assert_eq!(statuses(step), vec![(1, Ok(200))]);
        id
    }

    #[test]
    fn a_request_is_keep_alive_with_sanitized_headers() {
        let request = get("http://example.com:8080/v1/items?x=1")
            .header("Accept", "application/json")
            .header("x-api-key", "secret\r\nx-evil: 1");

        assert_eq!(
            request.to_bytes(),
            b"GET /v1/items?x=1 HTTP/1.1\r\nHost: example.com:8080\r\nAccept: application/json\r\nx-api-key: secretx-evil: 1\r\n\r\n"
        );
    }

    #[test]
    fn https_defaults_to_443_and_other_schemes_are_refused() {
        let request = get("https://api.example.com/v1");
        assert!(request.is_secure());
        assert_eq!(request.port(), 443);
        assert!(
            request
                .to_bytes()
                .starts_with(b"GET /v1 HTTP/1.1\r\nHost: api.example.com\r\n"),
            "the default port stays out of the Host header"
        );
        assert_eq!(
            Request::get("ftp://example.com/").unwrap_err(),
            Error::UnsupportedScheme("ftp".to_string())
        );
    }

    #[test]
    fn a_second_request_reuses_the_open_connection() {
        let mut client = client();
        let id = answered_once(&mut client);

        let step = client.send(2, &get("http://example.com/next"));

        assert!(
            connects(&step.actions).is_empty(),
            "no second connect while the connection is alive"
        );
        assert_eq!(sent(&step.actions).len(), 1);
        assert_eq!(sent(&step.actions)[0].0, id);
    }

    #[test]
    fn a_request_for_another_origin_replaces_the_connection() {
        let mut client = client();
        let first = answered_once(&mut client);

        let step = client.send(2, &get("http://other.example/"));

        assert_eq!(closes(&step.actions), vec![first]);
        assert_eq!(connects(&step.actions), vec![first + 1]);
    }

    #[test]
    fn a_reused_connection_that_dies_unanswered_is_retried_once_on_a_fresh_one() {
        let mut client = client();
        let first = answered_once(&mut client);
        client.send(2, &get("http://example.com/next"));

        let step = client.handle(&closed(first, None)).unwrap();
        assert!(
            step.replies.is_empty(),
            "a stale keep-alive is not an error"
        );
        let second = connects(&step.actions)[0];
        assert_ne!(second, first);
        let step = client.handle(&Event::Connected(second)).unwrap();
        assert_eq!(sent(&step.actions).len(), 1);

        let step = client.handle(&closed(second, Some("reset"))).unwrap();
        assert!(connects(&step.actions).is_empty(), "only one retry");
        assert_eq!(
            statuses(step),
            vec![(2, Err(Error::Closed(Some("reset".to_string()))))]
        );
    }

    #[test]
    fn a_fresh_connection_that_dies_unanswered_is_not_retried() {
        let mut client = client();
        let id = connects(&client.send(1, &get("http://example.com/")).actions)[0];
        client.handle(&Event::Connected(id)).unwrap();

        let step = client.handle(&closed(id, Some("reset"))).unwrap();

        assert!(connects(&step.actions).is_empty());
        assert_eq!(
            statuses(step),
            vec![(1, Err(Error::Closed(Some("reset".to_string()))))]
        );
    }

    #[test]
    fn a_failed_connect_fails_everything_waiting_instead_of_reconnecting() {
        let mut client = client();
        let id = connects(&client.send(1, &get("http://example.com/a")).actions)[0];
        client.send(2, &get("http://example.com/b"));

        let step = client.handle(&closed(id, Some("refused"))).unwrap();

        assert!(connects(&step.actions).is_empty());
        let refused = Err(Error::Closed(Some("refused".to_string())));
        assert_eq!(statuses(step), vec![(1, refused.clone()), (2, refused)]);
    }

    #[test]
    fn a_timeout_fails_the_request_and_the_next_one_starts_over() {
        let mut client = client();
        let id = connects(&client.send(1, &get("http://example.com/")).actions)[0];
        client.handle(&Event::Connected(id)).unwrap();

        let step = client.handle(&Event::Timer(TIMER)).unwrap();
        assert_eq!(closes(&step.actions), vec![id]);
        assert_eq!(statuses(step), vec![(1, Err(Error::Timeout))]);

        let step = client.send(2, &get("http://example.com/"));
        assert_eq!(connects(&step.actions), vec![id + 1]);
    }

    #[test]
    fn https_without_tls_fails_without_touching_the_network() {
        let mut client = client();

        let step = client.send(1, &get("https://example.com/"));

        assert!(step.actions.is_empty());
        assert_eq!(statuses(step), vec![(1, Err(Error::TlsUnavailable))]);
    }

    #[test]
    fn only_the_clients_own_streams_and_timer_are_claimed() {
        let mut client = client();
        let id = answered_once(&mut client);

        assert!(client.handle(&data(5, b"not ours")).is_none());
        assert!(client.handle(&Event::Timer(TIMER + 1)).is_none());
        let stale = client.handle(&closed(id + 3, None)).unwrap();
        assert!(stale.actions.is_empty() && stale.replies.is_empty());
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
