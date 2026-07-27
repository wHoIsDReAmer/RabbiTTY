use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::host::dir_name;
use super::*;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "rabbitty-plugin-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        Self(path)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn hello_component() -> Option<PathBuf> {
    ["debug", "release"]
        .iter()
        .map(|profile| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/wasm32-wasip2")
                .join(profile)
                .join("hello.wasm")
        })
        .find(|path| path.exists())
}

fn load_in(root: &TempRoot, policy: CapabilityPolicy<'_>) -> Option<LoadedPlugin> {
    let path = hello_component()?;
    let host = PluginHost::with_data_root(root.0.clone()).expect("engine");
    Some(
        host.load(&path, HashMap::new(), policy)
            .expect("hello plugin should load"),
    )
}

fn load(policy: CapabilityPolicy<'_>) -> Option<LoadedPlugin> {
    load_in(&TempRoot::new("load"), policy)
}

#[test]
fn manifest_and_contributions_round_trip() {
    let Some(plugin) = load(&grant_supported) else {
        return;
    };

    assert_eq!(plugin.info().name, "hello");
    assert_eq!(plugin.info().capabilities, vec![Capability::Notify]);
    assert_eq!(plugin.granted(), &[Capability::Notify]);

    let commands = &plugin.contributions().commands;
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].id, "hello.hi");
}

#[test]
fn granted_capability_lets_the_host_call_through() {
    let Some(mut plugin) = load(&grant_supported) else {
        return;
    };

    plugin.run_command("hello.hi").expect("command runs");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello from the hello plugin!".to_string(),
        }]
    );
}

#[test]
fn ungranted_capability_is_a_no_op() {
    let Some(mut plugin) = load(&grant_nothing) else {
        return;
    };

    plugin.run_command("hello.hi").expect("command still runs");

    assert!(
        plugin.drain_requests().is_empty(),
        "notify must be dropped when the capability is not granted"
    );
}

#[test]
fn events_reach_the_guest() {
    let Some(mut plugin) = load(&grant_supported) else {
        return;
    };

    plugin
        .on_event(Event::LineOutput(LineEvent {
            pane: 7,
            line: "a line mentioning hello".to_string(),
        }))
        .expect("event delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin saw 'hello' in the output".to_string(),
        }]
    );
}

#[test]
fn unmatched_events_produce_no_requests() {
    let Some(mut plugin) = load(&grant_supported) else {
        return;
    };

    plugin.on_event(Event::SessionStart(1)).expect("delivered");
    plugin
        .on_event(Event::LineOutput(LineEvent {
            pane: 1,
            line: "nothing of interest".to_string(),
        }))
        .expect("delivered");

    assert!(plugin.drain_requests().is_empty());
}

fn greedy() -> PluginInfo {
    PluginInfo {
        name: "greedy".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec![
            Capability::Notify,
            Capability::ReadConfig,
            Capability::Filesystem,
            Capability::WritePty,
            Capability::Network,
        ],
    }
}

#[test]
fn self_scoped_capabilities_are_granted_without_asking() {
    assert_eq!(
        grant_supported(&greedy()),
        vec![
            Capability::Notify,
            Capability::ReadConfig,
            Capability::Filesystem
        ]
    );
}

#[test]
fn command_execution_and_outbound_access_need_consent() {
    assert_eq!(
        requires_consent(&greedy()),
        vec![Capability::WritePty, Capability::Network]
    );
    assert!(!grant_supported(&greedy()).contains(&Capability::WritePty));
    assert!(!grant_supported(&greedy()).contains(&Capability::Network));
}

#[test]
fn consent_grants_only_what_was_agreed_to() {
    let granted = grant_with_consent(&greedy(), &[Capability::Network]);

    assert!(granted.contains(&Capability::Network));
    assert!(
        !granted.contains(&Capability::WritePty),
        "consenting to one capability must not imply the other"
    );
}

#[test]
fn consent_cannot_grant_what_was_never_requested() {
    let modest = PluginInfo {
        name: "modest".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec![Capability::Notify],
    };

    assert_eq!(
        grant_with_consent(&modest, &[Capability::Network, Capability::WritePty]),
        vec![Capability::Notify]
    );
}

#[test]
fn a_plugin_name_never_escapes_the_data_root() {
    assert_eq!(dir_name("../../etc").as_deref(), Some("etc"));
    assert_eq!(dir_name("..").as_deref(), None);
    assert_eq!(dir_name("a/b").as_deref(), Some("a_b"));
    assert_eq!(dir_name("").as_deref(), None);
    assert_eq!(dir_name("Hello Plugin").as_deref(), Some("hello_plugin"));
    assert_eq!(dir_name("hello").as_deref(), Some("hello"));
}

#[test]
fn filesystem_capability_opens_a_scoped_directory() {
    let root = TempRoot::new("fs-granted");
    let Some(plugin) = load_in(&root, &|_: &PluginInfo| vec![Capability::Filesystem]) else {
        return;
    };

    assert_eq!(plugin.granted(), &[Capability::Filesystem]);
    assert!(
        root.0.join("hello").is_dir(),
        "a granted filesystem capability should preopen the plugin's own directory"
    );
}

#[test]
fn without_the_capability_no_directory_is_created() {
    let root = TempRoot::new("fs-denied");
    let Some(_plugin) = load_in(&root, &grant_nothing) else {
        return;
    };

    assert!(!root.0.exists(), "nothing should be preopened");
}
