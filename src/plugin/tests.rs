use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::*;

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

fn load(policy: CapabilityPolicy) -> Option<LoadedPlugin> {
    let path = hello_component()?;
    let host = PluginHost::new().expect("engine");
    Some(
        host.load(&path, HashMap::new(), policy)
            .expect("hello plugin should load"),
    )
}

#[test]
fn manifest_and_contributions_round_trip() {
    let Some(plugin) = load(grant_supported) else {
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
    let Some(mut plugin) = load(grant_supported) else {
        return;
    };

    plugin.run_command("hello.hi").expect("command runs");

    assert_eq!(
        plugin.drain_requests(),
        vec![HostRequest::Notify {
            message: "hello from the hello plugin!".to_string(),
        }]
    );
}

#[test]
fn ungranted_capability_is_a_no_op() {
    let Some(mut plugin) = load(grant_nothing) else {
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
    let Some(mut plugin) = load(grant_supported) else {
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
        vec![HostRequest::Notify {
            message: "hello plugin saw 'hello' in the output".to_string(),
        }]
    );
}

#[test]
fn unmatched_events_produce_no_requests() {
    let Some(mut plugin) = load(grant_supported) else {
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

#[test]
fn reserved_capabilities_are_never_granted() {
    let info = PluginInfo {
        name: "greedy".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec![
            Capability::Network,
            Capability::Filesystem,
            Capability::Notify,
        ],
    };

    assert_eq!(grant_supported(&info), vec![Capability::Notify]);
}
