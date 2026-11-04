#![cfg(target_arch = "wasm32")]

use rabbitty_plugin_sdk::{
    Action, Capability, Command, ConnectRequest, ConnectTarget, Contributions, Event, IoFrame,
    LocalTarget, MenuContext, MenuItem, OutputPattern, Plugin, PluginInfo, PluginProfile,
    ProfileTarget, Query, SettingField, SettingKind, SshTarget, StatusItem, StatusText, TextPanel,
    Timer, export_plugin, read_config,
};

const STATUS_ID: &str = "hello.counter";
const PING_TIMER: u32 = 1;
const ECHO_STREAM: u32 = 7;
const ECHO_PROFILE: &str = "hello.echo";

#[derive(Default)]
struct HelloPlugin;

fn status(text: String) -> Action {
    Action::SetStatus(StatusText {
        id: STATUS_ID.to_string(),
        text,
    })
}

fn notify(text: String) -> Action {
    Action::Notify(text)
}

fn echo_tab() -> Vec<Action> {
    vec![Action::Connect(ConnectRequest {
        id: ECHO_STREAM,
        target: ConnectTarget::Tab("echo".to_string()),
    })]
}

fn to_echo(data: &[u8]) -> Vec<u8> {
    data.iter()
        .flat_map(|&byte| match byte {
            b'\r' => vec![b'\r', b'\n'],
            other => vec![other],
        })
        .collect()
}

fn profiles() -> Vec<PluginProfile> {
    vec![
        PluginProfile {
            id: "hello.local".to_string(),
            name: "Hello shell".to_string(),
            subtitle: Some("from the hello plugin".to_string()),
            icon: None,
            target: ProfileTarget::Local(LocalTarget {
                program: None,
                args: vec![],
            }),
        },
        PluginProfile {
            id: "hello.ssh".to_string(),
            name: "Hello SSH".to_string(),
            subtitle: Some("example.invalid".to_string()),
            icon: None,
            target: ProfileTarget::Ssh(SshTarget {
                host: "example.invalid".to_string(),
                port: 22,
                user: "demo".to_string(),
                identity_file: None,
            }),
        },
        PluginProfile {
            id: ECHO_PROFILE.to_string(),
            name: "Hello echo".to_string(),
            subtitle: Some("a tab the hello plugin answers".to_string()),
            icon: None,
            target: ProfileTarget::Plugin(ECHO_PROFILE.to_string()),
        },
    ]
}

impl Plugin for HelloPlugin {
    fn manifest() -> PluginInfo {
        PluginInfo {
            name: "hello".to_string(),
            version: "0.1.0".to_string(),
            description: Some("Exercises every surface of the Rabbitty plugin ABI.".to_string()),
            author: Some("Rabbitty".to_string()),
            homepage: Some("https://github.com/wHoIsDReAmer/RabbiTTY".to_string()),
            capabilities: vec![
                Capability::Notify,
                Capability::ReadConfig,
                Capability::ReadScreen,
                Capability::Network,
                Capability::OpenUrl,
                Capability::Control,
                Capability::Clipboard,
            ],
        }
    }

    fn contributions() -> Contributions {
        Contributions {
            commands: vec![
                Command {
                    id: "hello.hi".to_string(),
                    title: "Say hi".to_string(),
                    default_key: Some("Ctrl+Shift+H".to_string()),
                },
                Command {
                    id: "hello.ping".to_string(),
                    title: "Ping (pong after 10 ms)".to_string(),
                    default_key: None,
                },
                Command {
                    id: "hello.fail".to_string(),
                    title: "Report a failure".to_string(),
                    default_key: None,
                },
                Command {
                    id: "hello.boom".to_string(),
                    title: "Crash on purpose".to_string(),
                    default_key: None,
                },
            ],
            output_patterns: vec![
                OutputPattern {
                    id: "hello.greeting".to_string(),
                    regex: "hello".to_string(),
                    clickable: false,
                },
                OutputPattern {
                    id: "hello.issue".to_string(),
                    regex: r"#\d+".to_string(),
                    clickable: true,
                },
            ],
            settings: vec![
                SettingField {
                    key: "greeting".to_string(),
                    label: "Greeting".to_string(),
                    kind: SettingKind::Text,
                    default_value: "hello".to_string(),
                },
                SettingField {
                    key: "loud".to_string(),
                    label: "Shout it".to_string(),
                    kind: SettingKind::Toggle,
                    default_value: "false".to_string(),
                },
                SettingField {
                    key: "token".to_string(),
                    label: "Token".to_string(),
                    kind: SettingKind::Secret,
                    default_value: String::new(),
                },
            ],
            menu_items: vec![
                MenuItem {
                    id: "hello.hi".to_string(),
                    title: "Say hi".to_string(),
                    context: MenuContext::Terminal,
                },
                MenuItem {
                    id: "hello.readconfig".to_string(),
                    title: "Read my greeting".to_string(),
                    context: MenuContext::Tab,
                },
            ],
            status_items: vec![StatusItem {
                id: STATUS_ID.to_string(),
                text: "hello: 0".to_string(),
                tooltip: Some("Panes opened since launch".to_string()),
                command: Some("hello.hi".to_string()),
            }],
        }
    }

    fn init(&mut self) -> Result<Vec<Action>, String> {
        Ok(vec![Action::SetProfiles(profiles())])
    }

    fn shutdown(&mut self) -> Vec<Action> {
        vec![notify("hello plugin shutting down".to_string())]
    }

    fn on_event(&mut self, ev: Event) -> Vec<Action> {
        match ev {
            Event::SessionStart(pane) => vec![
                notify(format!("hello plugin saw pane {pane} open")),
                status(format!("hello: pane {pane}")),
                Action::Query(Query::Panes),
            ],
            Event::SessionClose(pane) => {
                vec![notify(format!("hello plugin saw pane {pane} close"))]
            }
            Event::OutputMatched(matched) => vec![notify(format!(
                "hello plugin matched {} in pane {}",
                matched.pattern, matched.pane
            ))],
            Event::MatchActivated(matched) => {
                let start = matched.start as usize;
                let end = matched.end as usize;
                let text = matched.line.get(start..end).unwrap_or_default();
                vec![Action::OpenUrl(format!(
                    "https://example.com/issues/{}",
                    text.trim_matches('#')
                ))]
            }
            Event::CwdChanged(cwd) => vec![status(format!("cwd: {}", cwd.path))],
            Event::TitleChanged(title) => vec![notify(format!(
                "hello plugin saw pane {} retitled to {}",
                title.pane, title.title
            ))],
            Event::PaneFocused(pane) => vec![status(format!("focus: {pane}"))],
            Event::ActiveTabChanged(tab) => vec![status(format!("tab: {tab}"))],
            Event::SelectionChanged(pane) => vec![Action::Query(Query::Selection(pane))],
            Event::Selection(selection) => vec![notify(format!(
                "hello plugin saw {} chars selected in pane {}",
                selection.text.chars().count(),
                selection.pane
            ))],
            Event::MenuActivated(menu) => {
                let picked = menu.selection.unwrap_or_else(|| "<nothing>".to_string());
                vec![notify(format!(
                    "hello plugin menu {} in pane {} over {picked}",
                    menu.item, menu.pane
                ))]
            }
            Event::Bell(pane) => vec![notify(format!("hello plugin heard a bell in pane {pane}"))],
            Event::SettingChanged(setting) => vec![notify(format!(
                "hello plugin saw {} change to {}",
                setting.key, setting.value
            ))],
            Event::CommandFinished(command) => vec![status(match command.exit {
                Some(code) => format!("exit {code}: {} lines", command.output.count),
                None => format!("done: {} lines", command.output.count),
            })],
            Event::Timer(PING_TIMER) => vec![notify("pong".to_string())],
            Event::Panes(list) => vec![status(format!("{} panes", list.len()))],
            Event::ProfileOpened(key) if key == ECHO_PROFILE => echo_tab(),
            Event::Connected(ECHO_STREAM) => vec![Action::Send(IoFrame {
                id: ECHO_STREAM,
                data: b"hello echo: type and it comes back\r\n".to_vec(),
            })],
            Event::Data(frame) if frame.id == ECHO_STREAM => vec![Action::Send(IoFrame {
                id: ECHO_STREAM,
                data: to_echo(&frame.data),
            })],
            Event::Timer(_)
            | Event::Scrollback(_)
            | Event::Connected(_)
            | Event::Data(_)
            | Event::Closed(_)
            | Event::TabOpened(_)
            | Event::ProfileOpened(_)
            | Event::Resized(_) => Vec::new(),
        }
    }

    fn run_command(&mut self, id: &str) -> Result<Vec<Action>, String> {
        match id {
            "hello.hi" => Ok(vec![notify("hello from the hello plugin!".to_string())]),
            "hello.ping" => Ok(vec![Action::Schedule(Timer {
                id: PING_TIMER,
                after_ms: 10,
                repeat: false,
            })]),
            "hello.readconfig" => {
                let greeting = read_config("greeting").unwrap_or_else(|| "<none>".to_string());
                Ok(vec![notify(format!(
                    "hello plugin read greeting={greeting}"
                ))])
            }
            "hello.readtoken" => {
                let token = read_config("token").unwrap_or_else(|| "<none>".to_string());
                Ok(vec![notify(format!("hello plugin read token={token}"))])
            }
            "hello.panel" => Ok(vec![Action::ShowText(TextPanel {
                title: "hello".to_string(),
                body: "a panel from the hello plugin".to_string(),
            })]),
            "hello.tab" => Ok(echo_tab()),
            "hello.copy" => Ok(vec![Action::SetClipboard(
                "copied by the hello plugin".to_string(),
            )]),
            "hello.grow" => {
                let mut grown = Self::contributions();
                grown.commands.push(Command {
                    id: "hello.extra".to_string(),
                    title: "Added at runtime".to_string(),
                    default_key: None,
                });
                Ok(vec![Action::SetContributions(grown)])
            }
            "hello.extra" => Ok(vec![notify(
                "hello plugin ran a runtime command".to_string(),
            )]),
            "hello.stall" => {
                std::thread::sleep(std::time::Duration::from_secs(3));
                Ok(Vec::new())
            }
            "hello.hog" => {
                let mut blocks: Vec<Vec<u8>> = Vec::new();
                for _ in 0..512 {
                    blocks.push(Vec::with_capacity(1024 * 1024));
                }
                Ok(vec![notify(format!("allocated {} blocks", blocks.len()))])
            }
            "hello.boom" => panic!("intentional panic, for host failure-isolation tests"),
            "hello.fail" => Err("intentional failure, for host error-path tests".to_string()),
            other => Err(format!("unknown command: {other}")),
        }
    }
}

export_plugin!(HelloPlugin);
