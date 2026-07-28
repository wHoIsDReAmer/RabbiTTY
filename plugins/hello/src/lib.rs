#![cfg(target_arch = "wasm32")]

//! Minimal example plugin exercising the Rabbitty plugin ABI (`wit/world.wit`).

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use crate::rabbitty::plugin::host;
use crate::rabbitty::plugin::types::{
    Capability, Command, OutputPattern, SettingField, SettingKind,
};

struct HelloPlugin;

impl Guest for HelloPlugin {
    fn manifest() -> PluginInfo {
        PluginInfo {
            name: "hello".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![
                Capability::Notify,
                Capability::ReadConfig,
                Capability::Network,
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
                },
                Command {
                    id: "hello.fail".to_string(),
                    title: "Report a failure".to_string(),
                },
                Command {
                    id: "hello.boom".to_string(),
                    title: "Crash on purpose".to_string(),
                },
            ],
            output_patterns: vec![OutputPattern {
                id: "hello.greeting".to_string(),
                regex: "hello".to_string(),
            }],
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
        })
    }

    fn on_event(ev: Event) -> Result<(), String> {
        match ev {
            Event::SessionStart(pane) => {
                host::notify(&format!("hello plugin saw pane {pane} open"));
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
            Event::CwdChanged(_) => {}
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
            "hello.boom" => panic!("intentional panic, for host failure-isolation tests"),
            "hello.fail" => Err("intentional failure, for host error-path tests".to_string()),
            other => Err(format!("unknown command: {other}")),
        }
    }
}

export!(HelloPlugin);
