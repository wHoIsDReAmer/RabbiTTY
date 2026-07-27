#![cfg(target_arch = "wasm32")]

//! Minimal example plugin exercising the Rabbitty plugin ABI (`wit/world.wit`).

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use crate::rabbitty::plugin::host;
use crate::rabbitty::plugin::types::{Capability, Command};

struct HelloPlugin;

impl Guest for HelloPlugin {
    fn manifest() -> PluginInfo {
        PluginInfo {
            name: "hello".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![Capability::Notify],
        }
    }

    fn init() {}

    fn shutdown() {
        host::notify("hello plugin shutting down");
    }

    fn contributions() -> Contributions {
        Contributions {
            commands: vec![Command {
                id: "hello.hi".to_string(),
                title: "Say hi".to_string(),
                default_key: None,
            }],
            menu_items: vec![],
        }
    }

    fn on_event(ev: Event) {
        match ev {
            Event::SessionStart(pane) => {
                host::notify(&format!("hello plugin saw pane {pane} open"));
            }
            Event::SessionClose(pane) => {
                host::notify(&format!("hello plugin saw pane {pane} close"));
            }
            Event::LineOutput(line) if line.line.contains("hello") => {
                host::notify("hello plugin saw 'hello' in the output");
            }
            _ => {}
        }
    }

    fn run_command(id: String) {
        match id.as_str() {
            "hello.hi" => host::notify("hello from the hello plugin!"),
            "hello.boom" => panic!("intentional panic, for host failure-isolation tests"),
            _ => {}
        }
    }
}

export!(HelloPlugin);
