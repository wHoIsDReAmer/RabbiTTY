use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::time::{Duration, Instant};

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::plugin::{ConnectTarget, Event, IoClosed, IoFrame, TcpTarget};

use super::Message;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::gui) struct LinkKey {
    pub plugin: String,
    pub id: u32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Alarm {
    due: Instant,
    plugin: String,
    id: u32,
    repeat: Option<Duration>,
}

#[derive(Default)]
pub(in crate::gui) struct PluginRuntime {
    alarms: BinaryHeap<Reverse<Alarm>>,
    links: HashMap<LinkKey, mpsc::UnboundedSender<Vec<u8>>>,
}

impl PluginRuntime {
    pub fn schedule(&mut self, plugin: &str, timer: crate::plugin::Timer, now: Instant) {
        self.cancel(plugin, timer.id);
        let interval = Duration::from_millis(timer.after_ms);
        self.alarms.push(Reverse(Alarm {
            due: now + interval,
            plugin: plugin.to_string(),
            id: timer.id,
            repeat: timer.repeat.then_some(interval),
        }));
    }

    pub fn cancel(&mut self, plugin: &str, id: u32) {
        self.alarms
            .retain(|Reverse(alarm)| !(alarm.plugin == plugin && alarm.id == id));
    }

    pub fn next_due(&self) -> Option<Instant> {
        self.alarms.peek().map(|Reverse(alarm)| alarm.due)
    }

    pub fn fire_due(&mut self, now: Instant) -> Vec<(String, u32)> {
        let mut fired = Vec::new();
        while let Some(Reverse(alarm)) = self.alarms.peek()
            && alarm.due <= now
        {
            let Some(Reverse(alarm)) = self.alarms.pop() else {
                break;
            };
            fired.push((alarm.plugin.clone(), alarm.id));
            if let Some(interval) = alarm.repeat {
                self.alarms.push(Reverse(Alarm {
                    due: alarm.due + interval,
                    ..alarm
                }));
            }
        }
        fired
    }

    pub fn connect(&mut self, key: LinkKey, target: ConnectTarget) -> iced::Task<Message> {
        let (tx, rx) = mpsc::unbounded();
        self.links.insert(key.clone(), tx);
        iced::Task::stream(link_stream(key, target, rx))
    }

    pub fn send(&self, key: &LinkKey, data: Vec<u8>) -> bool {
        self.links
            .get(key)
            .is_some_and(|tx| tx.unbounded_send(data).is_ok())
    }

    pub fn close(&mut self, key: &LinkKey) {
        self.links.remove(key);
    }

    pub fn drop_plugin(&mut self, plugin: &str) {
        self.links.retain(|key, _| key.plugin != plugin);
        self.alarms.retain(|Reverse(alarm)| alarm.plugin != plugin);
    }
}

const MAX_FRAME: usize = 64 * 1024;

fn link_stream(
    key: LinkKey,
    target: ConnectTarget,
    outbound: mpsc::UnboundedReceiver<Vec<u8>>,
) -> impl iced::futures::Stream<Item = Message> {
    iced::stream::channel(32, async move |mut out| {
        let plugin = key.plugin.clone();
        let id = key.id;
        let emit = |event| Message::PluginIo {
            plugin: plugin.clone(),
            event,
        };

        let closed = match open(target).await {
            Ok(stream) => {
                let _ = out.send(emit(Event::Connected(id))).await;
                pump(stream, outbound, id, &mut out, &emit).await
            }
            Err(reason) => IoClosed {
                id,
                reason: Some(reason),
            },
        };
        let _ = out.send(emit(Event::Closed(closed))).await;
    })
}

enum Link {
    Tcp(tokio::net::TcpStream),
    #[cfg(unix)]
    Local(tokio::net::UnixStream),
    #[cfg(windows)]
    Local(tokio::net::windows::named_pipe::NamedPipeClient),
}

async fn open(target: ConnectTarget) -> Result<Link, String> {
    match target {
        ConnectTarget::Tcp(TcpTarget { host, port }) => {
            tokio::net::TcpStream::connect((host, port))
                .await
                .map(Link::Tcp)
                .map_err(|err| err.to_string())
        }
        ConnectTarget::Local(name) => open_local(&name).await,
    }
}

#[cfg(unix)]
async fn open_local(name: &str) -> Result<Link, String> {
    let dir = std::env::var_os("TMPDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let mut last = String::from("no socket found");
    for suffix in 0..10 {
        let path = dir.join(format!("{name}-{suffix}"));
        match tokio::net::UnixStream::connect(&path).await {
            Ok(stream) => return Ok(Link::Local(stream)),
            Err(err) => last = format!("{}: {err}", path.display()),
        }
    }
    Err(last)
}

#[cfg(windows)]
async fn open_local(name: &str) -> Result<Link, String> {
    let mut last = String::from("no pipe found");
    for suffix in 0..10 {
        let path = format!(r"\\.\pipe\{name}-{suffix}");
        match tokio::net::windows::named_pipe::ClientOptions::new().open(&path) {
            Ok(pipe) => return Ok(Link::Local(pipe)),
            Err(err) => last = format!("{path}: {err}"),
        }
    }
    Err(last)
}

async fn pump(
    link: Link,
    mut outbound: mpsc::UnboundedReceiver<Vec<u8>>,
    id: u32,
    out: &mut mpsc::Sender<Message>,
    emit: &impl Fn(Event) -> Message,
) -> IoClosed {
    let (mut reader, mut writer): (
        Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        Box<dyn tokio::io::AsyncWrite + Unpin + Send>,
    ) = match link {
        Link::Tcp(stream) => {
            let (r, w) = stream.into_split();
            (Box::new(r), Box::new(w))
        }
        Link::Local(stream) => {
            let (r, w) = tokio::io::split(stream);
            (Box::new(r), Box::new(w))
        }
    };

    let mut buf = vec![0u8; MAX_FRAME];
    loop {
        tokio::select! {
            read = reader.read(&mut buf) => match read {
                Ok(0) => return IoClosed { id, reason: None },
                Ok(n) => {
                    let frame = IoFrame { id, data: buf[..n].to_vec() };
                    if out.send(emit(Event::Data(frame))).await.is_err() {
                        return IoClosed { id, reason: None };
                    }
                }
                Err(err) => return IoClosed { id, reason: Some(err.to_string()) },
            },
            data = outbound.next() => match data {
                Some(data) => {
                    if let Err(err) = writer.write_all(&data).await {
                        return IoClosed { id, reason: Some(err.to_string()) };
                    }
                }
                None => return IoClosed { id, reason: None },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timer(id: u32, after_ms: u64, repeat: bool) -> crate::plugin::Timer {
        crate::plugin::Timer {
            id,
            after_ms,
            repeat,
        }
    }

    #[test]
    fn alarms_fire_in_deadline_order_regardless_of_insertion() {
        let mut rt = PluginRuntime::default();
        let t0 = Instant::now();
        rt.schedule("a", timer(1, 300, false), t0);
        rt.schedule("b", timer(2, 100, false), t0);
        rt.schedule("a", timer(3, 200, false), t0);

        assert_eq!(rt.next_due(), Some(t0 + Duration::from_millis(100)));
        assert_eq!(
            rt.fire_due(t0 + Duration::from_millis(250)),
            vec![("b".into(), 2), ("a".into(), 3)]
        );
        assert_eq!(rt.fire_due(t0 + Duration::from_millis(250)), vec![]);
        assert_eq!(
            rt.fire_due(t0 + Duration::from_millis(300)),
            vec![("a".into(), 1)]
        );
        assert_eq!(rt.next_due(), None);
    }

    #[test]
    fn a_repeating_alarm_reschedules_from_its_due_time_not_from_now() {
        let mut rt = PluginRuntime::default();
        let t0 = Instant::now();
        rt.schedule("a", timer(1, 100, true), t0);

        assert_eq!(rt.fire_due(t0 + Duration::from_millis(150)).len(), 1);
        assert_eq!(rt.next_due(), Some(t0 + Duration::from_millis(200)));
    }

    #[test]
    fn rescheduling_the_same_id_replaces_the_earlier_alarm() {
        let mut rt = PluginRuntime::default();
        let t0 = Instant::now();
        rt.schedule("a", timer(1, 100, false), t0);
        rt.schedule("a", timer(1, 500, false), t0);

        assert_eq!(rt.fire_due(t0 + Duration::from_millis(200)), vec![]);
        assert_eq!(
            rt.fire_due(t0 + Duration::from_millis(500)),
            vec![("a".into(), 1)]
        );
    }

    #[test]
    fn cancelling_one_plugin_leaves_another_plugins_alarm_with_the_same_id() {
        let mut rt = PluginRuntime::default();
        let t0 = Instant::now();
        rt.schedule("a", timer(7, 100, false), t0);
        rt.schedule("b", timer(7, 100, false), t0);
        rt.cancel("a", 7);

        assert_eq!(
            rt.fire_due(t0 + Duration::from_millis(100)),
            vec![("b".into(), 7)]
        );
    }

    #[test]
    fn dropping_a_plugin_clears_its_alarms_and_links() {
        let mut rt = PluginRuntime::default();
        let t0 = Instant::now();
        rt.schedule("a", timer(1, 10, true), t0);
        rt.schedule("b", timer(1, 10, true), t0);
        let (tx, _rx) = mpsc::unbounded();
        rt.links.insert(
            LinkKey {
                plugin: "a".into(),
                id: 1,
            },
            tx,
        );
        rt.drop_plugin("a");

        assert!(!rt.send(
            &LinkKey {
                plugin: "a".into(),
                id: 1
            },
            vec![1]
        ));
        assert_eq!(
            rt.fire_due(t0 + Duration::from_millis(10)),
            vec![("b".into(), 1)]
        );
    }
}
