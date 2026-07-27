use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::host::dir_name;
use super::registry;
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
    let host = PluginHost::with_root(root.0.clone()).expect("engine");
    Some(
        host.load("hello", &path, HashMap::new(), policy)
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
        root.0.join("hello/data").is_dir(),
        "a granted filesystem capability should preopen the plugin's data directory, \
         not the install directory that holds its component"
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

#[test]
fn a_panicking_command_retires_the_plugin() {
    let Some(mut plugin) = load(&grant_supported) else {
        return;
    };

    assert!(plugin.run_command("hello.boom").is_err());
    assert!(
        plugin.failure().is_some(),
        "a trapped instance must be marked retired"
    );

    let err = plugin
        .run_command("hello.hi")
        .expect_err("a retired instance cannot be re-entered");
    assert!(
        err.to_string().starts_with("plugin retired:"),
        "later calls should report the original failure, not a re-entry error: {err}"
    );
}

fn install(root: &TempRoot, id: &str) -> bool {
    let Some(component) = hello_component() else {
        return false;
    };
    let dir = root.0.join(id);
    std::fs::create_dir_all(&dir).expect("install dir");
    std::fs::copy(component, dir.join(registry::COMPONENT_FILE)).expect("copy component");
    true
}

fn registry_in(root: &TempRoot) -> PluginRegistry {
    registry_with(root, crate::config::plugins::PluginsConfig::new())
}

fn registry_with(
    root: &TempRoot,
    settings: crate::config::plugins::PluginsConfig,
) -> PluginRegistry {
    PluginRegistry::new(
        PluginHost::with_root(root.0.clone()).expect("engine"),
        settings,
    )
}

#[test]
fn every_installed_plugin_is_discovered() {
    let root = TempRoot::new("discover");
    if !install(&root, "alpha") || !install(&root, "beta") {
        return;
    }
    std::fs::create_dir_all(root.0.join("no-component")).expect("stray dir");

    let mut registry = registry_in(&root);
    registry.load_all();

    assert_eq!(registry.ids().collect::<Vec<_>>(), vec!["alpha", "beta"]);
    assert_eq!(registry.status("alpha"), Some(Status::Ready));
    assert_eq!(
        registry.status("no-component"),
        None,
        "a directory without a component is not a plugin"
    );
}

#[test]
fn a_broken_component_does_not_stop_the_others() {
    let root = TempRoot::new("isolation");
    if !install(&root, "good") {
        return;
    }
    let broken = root.0.join("broken");
    std::fs::create_dir_all(&broken).expect("dir");
    std::fs::write(broken.join(registry::COMPONENT_FILE), b"not wasm").expect("write");

    let mut registry = registry_in(&root);
    registry.load_all();

    assert_eq!(registry.status("good"), Some(Status::Ready));
    assert!(matches!(
        registry.status("broken"),
        Some(Status::Retired(_))
    ));
    assert_eq!(registry.ready_mut().count(), 1);
}

#[test]
fn a_trapped_plugin_is_retired_and_leaves_the_rest_running() {
    let root = TempRoot::new("retire");
    if !install(&root, "alpha") || !install(&root, "beta") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let alpha = registry.get_mut("alpha").expect("alpha ready");
    assert!(alpha.run_command("hello.boom").is_err());

    let retired = registry.retire_failed();
    assert_eq!(retired.len(), 1);
    assert_eq!(retired[0].0, "alpha");

    assert!(matches!(registry.status("alpha"), Some(Status::Retired(_))));
    assert!(registry.get_mut("alpha").is_none());
    assert_eq!(registry.status("beta"), Some(Status::Ready));
    assert_eq!(registry.ready_mut().count(), 1);
}

#[test]
fn disabling_hides_a_plugin_and_enabling_brings_it_back() {
    let root = TempRoot::new("toggle");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(registry.disable("alpha"));
    assert_eq!(registry.status("alpha"), Some(Status::Disabled));
    assert!(registry.get_mut("alpha").is_none());

    registry.enable("alpha").expect("reload");
    assert_eq!(registry.status("alpha"), Some(Status::Ready));
    assert!(registry.get_mut("alpha").is_some());
}

#[test]
fn a_retired_plugin_can_be_revived_by_enabling_it() {
    let root = TempRoot::new("revive");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry
        .get_mut("alpha")
        .expect("ready")
        .run_command("hello.boom")
        .expect_err("panics");
    registry.retire_failed();

    registry.enable("alpha").expect("reload");

    assert_eq!(registry.status("alpha"), Some(Status::Ready));
    registry
        .get_mut("alpha")
        .expect("ready again")
        .run_command("hello.hi")
        .expect("a fresh instance works");
}

#[test]
fn a_plugin_disabled_in_config_is_not_instantiated() {
    let root = TempRoot::new("cfg-disabled");
    if !install(&root, "alpha") {
        return;
    }
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: false,
            consented: Vec::new(),
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    assert_eq!(registry.status("alpha"), Some(Status::Disabled));
    assert_eq!(registry.ready_mut().count(), 0);
}

#[test]
fn toggling_is_recorded_for_persistence() {
    let root = TempRoot::new("cfg-toggle");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.disable("alpha");

    assert_eq!(
        registry.settings().get("alpha").map(|s| s.enabled),
        Some(false),
        "disable must be written back so it survives a restart"
    );

    registry.enable("alpha").expect("reload");
    assert_eq!(
        registry.settings().get("alpha").map(|s| s.enabled),
        Some(true)
    );
}

#[test]
fn consent_from_config_grants_the_capability() {
    let root = TempRoot::new("cfg-consent");
    if !install(&root, "alpha") {
        return;
    }
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: true,
            consented: vec!["network".to_string()],
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    assert_eq!(registry.status("alpha"), Some(Status::Ready));
    assert!(
        !registry
            .get_mut("alpha")
            .expect("ready")
            .granted()
            .contains(&Capability::Network),
        "hello never requests network, so consent alone must not grant it"
    );
}

#[test]
fn recording_consent_is_kept_for_persistence() {
    let root = TempRoot::new("cfg-record");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.consent("alpha", Capability::Network);
    registry.consent("alpha", Capability::Network);

    assert_eq!(
        registry
            .settings()
            .get("alpha")
            .map(|s| s.consented.clone()),
        Some(vec!["network".to_string()]),
        "consent is recorded once, by its WIT name"
    );
}

#[test]
fn plugin_settings_round_trip_through_toml() {
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: false,
            consented: vec!["network".to_string(), "write-pty".to_string()],
        },
    );

    let text = toml::to_string_pretty(&settings).expect("serialize");
    let back: crate::config::plugins::PluginsConfig = toml::from_str(&text).expect("deserialize");

    assert_eq!(back, settings);
}

#[test]
fn an_unlisted_plugin_defaults_to_enabled_without_consent() {
    let defaults = crate::config::plugins::PluginSettings::default();

    assert!(defaults.enabled);
    assert!(defaults.consented.is_empty());
}

#[test]
fn capability_names_match_the_wit_spelling() {
    for cap in [
        Capability::WritePty,
        Capability::ReadConfig,
        Capability::Notify,
        Capability::Network,
        Capability::Filesystem,
    ] {
        let name = capability_name(cap);
        assert_eq!(
            capability_from_name(name),
            Some(cap),
            "round trip for {name}"
        );
    }
    assert_eq!(capability_name(Capability::WritePty), "write-pty");
    assert_eq!(capability_from_name("nonsense"), None);
}
