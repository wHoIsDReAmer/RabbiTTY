// This plugin is a WASM component; it is only meaningful for wasm targets.
// Off-wasm (e.g. the host's `cargo test --workspace`) it compiles to an empty
// crate so it never breaks native builds.
#![cfg(target_arch = "wasm32")]

//! Minimal example plugin exercising the Rabbitty plugin ABI (`wit/world.wit`).
//! Not a real feature — a "hello world" fixture that proves the guest bindings
//! compile against our WIT and the host can drive them end to end.

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
        if let Event::LineOutput(line) = ev
            && line.line.contains("hello")
        {
            host::notify("hello plugin saw 'hello' in the output");
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
