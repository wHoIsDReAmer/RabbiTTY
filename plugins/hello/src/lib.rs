#![cfg(target_arch = "wasm32")]

//! Minimal example plugin exercising the Rabbitty plugin ABI (`wit/world.wit`).

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use crate::rabbitty::plugin::host;
use crate::rabbitty::plugin::types::{
    Capability, Command, LocalTarget, MenuContext, MenuItem, OutputPattern, ProfileTarget,
    SettingField, SettingKind, SshTarget, StatusItem,
};

struct HelloPlugin;

impl Guest for HelloPlugin {
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
                Capability::Network,
                Capability::OpenUrl,
            ],
        }
    }

    fn init() -> Result<(), String> {
        Ok(())
    }

    fn shutdown() -> Result<(), String> {
        host::notify("hello plugin shutting down");
        Ok(())
    }

    fn contributions() -> Result<Contributions, String> {
        Ok(Contributions {
            commands: vec![
                Command {
                    id: "hello.hi".to_string(),
                    title: "Say hi".to_string(),
                    default_key: Some("Ctrl+Shift+H".to_string()),
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
                id: "hello.counter".to_string(),
                text: "hello: 0".to_string(),
                tooltip: Some("Panes opened since launch".to_string()),
                command: Some("hello.hi".to_string()),
            }],
        })
    }

    fn list_profiles() -> Result<Vec<PluginProfile>, String> {
        if host::read_config("slow").as_deref() == Some("true") {
            std::thread::sleep(std::time::Duration::from_secs(3));
        }

        Ok(vec![
            PluginProfile {
                id: "hello.local".to_string(),
                name: "Hello shell".to_string(),
                subtitle: Some("from the hello plugin".to_string()),
                icon: None,
                // A one-shot command would exit before the tab is even painted,
                // so the demo profile opens the user's shell instead.
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
        ])
    }

    fn on_event(ev: Event) -> Result<(), String> {
        match ev {
            Event::SessionStart(pane) => {
                host::notify(&format!("hello plugin saw pane {pane} open"));
                host::set_status("hello.counter", &format!("hello: pane {pane}"));
            }
            Event::SessionClose(pane) => {
                host::notify(&format!("hello plugin saw pane {pane} close"));
            }
            Event::OutputMatched(matched) => {
                host::notify(&format!(
                    "hello plugin matched {} in pane {}",
                    matched.pattern, matched.pane
                ));
            }
            Event::MatchActivated(matched) => {
                let start = matched.start as usize;
                let end = matched.end as usize;
                let text = matched.line.get(start..end).unwrap_or_default();
                host::open_url(&format!(
                    "https://example.com/issues/{}",
                    text.trim_matches('#')
                ));
            }
            Event::CwdChanged(cwd) => {
                host::set_status("hello.counter", &format!("cwd: {}", cwd.path));
            }
            Event::TitleChanged(title) => {
                host::notify(&format!(
                    "hello plugin saw pane {} retitled to {}",
                    title.pane, title.title
                ));
            }
            Event::PaneFocused(pane) => {
                host::set_status("hello.counter", &format!("focus: {pane}"));
            }
            Event::ActiveTabChanged(tab) => {
                host::set_status("hello.counter", &format!("tab: {tab}"));
            }
            Event::SelectionChanged(selection) => {
                host::notify(&format!(
                    "hello plugin saw {} chars selected in pane {}",
                    selection.text.chars().count(),
                    selection.pane
                ));
            }
            Event::MenuActivated(menu) => {
                let picked = menu.selection.unwrap_or_else(|| "<nothing>".to_string());
                host::notify(&format!(
                    "hello plugin menu {} in pane {} over {picked}",
                    menu.item, menu.pane
                ));
            }
            Event::Bell(pane) => {
                host::notify(&format!("hello plugin heard a bell in pane {pane}"));
            }
            Event::SettingChanged(setting) => {
                host::notify(&format!(
                    "hello plugin saw {} change to {}",
                    setting.key, setting.value
                ));
            }
        }
        Ok(())
    }

    fn run_command(id: String) -> Result<(), String> {
        match id.as_str() {
            "hello.hi" => {
                host::notify("hello from the hello plugin!");
                Ok(())
            }
            "hello.readconfig" => {
                let greeting =
                    host::read_config("greeting").unwrap_or_else(|| "<none>".to_string());
                host::notify(&format!("hello plugin read greeting={greeting}"));
                Ok(())
            }
            "hello.hog" => {
                let mut blocks: Vec<Vec<u8>> = Vec::new();
                for _ in 0..512 {
                    blocks.push(Vec::with_capacity(1024 * 1024));
                }
                host::notify(&format!("allocated {} blocks", blocks.len()));
                Ok(())
            }
            "hello.boom" => panic!("intentional panic, for host failure-isolation tests"),
            "hello.fail" => Err("intentional failure, for host error-path tests".to_string()),
            other => Err(format!("unknown command: {other}")),
        }
    }
}

export!(HelloPlugin);
