use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::host::{LoadedPlugin, dir_name};
use super::policy::{
    CapabilityPolicy, capability_from_name, capability_name, grant_with_consent, requires_consent,
};

fn auto(info: &PluginInfo) -> Vec<Capability> {
    grant_with_consent(info, &[])
}

fn nothing(_info: &PluginInfo) -> Vec<Capability> {
    Vec::new()
}
use super::rabbitty::plugin::types::{CwdEvent, MatchEvent, MenuEvent, ProfileTarget};
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
    let Some(plugin) = load(&auto) else {
        return;
    };

    assert_eq!(plugin.info().name, "hello");
    assert_eq!(
        plugin.info().capabilities,
        vec![
            Capability::Notify,
            Capability::ReadConfig,
            Capability::Network,
            Capability::OpenUrl
        ]
    );
    assert_eq!(
        plugin.granted(),
        &[Capability::Notify, Capability::ReadConfig],
        "network and open-url are requested but not consented to, so they stay ungranted"
    );

    let commands = &plugin.contributions().commands;
    assert_eq!(commands.len(), 3);
    assert_eq!(commands[0].id, "hello.hi");
    assert_eq!(commands[0].title, "Say hi");
}

#[test]
fn granted_capability_lets_the_host_call_through() {
    let Some(mut plugin) = load(&auto) else {
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
    let Some(mut plugin) = load(&nothing) else {
        return;
    };

    plugin.run_command("hello.hi").expect("command still runs");

    assert!(
        plugin.drain_requests().is_empty(),
        "notify must be dropped when the capability is not granted"
    );
}

#[test]
fn session_events_reach_the_guest() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    plugin
        .on_event(Event::SessionStart(7))
        .expect("event delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![
            PluginRequest::Notify {
                message: "hello plugin saw pane 7 open".to_string(),
            },
            PluginRequest::SetStatus {
                id: "hello.counter".to_string(),
                text: "hello: pane 7".to_string(),
            },
        ],
        "every host call the guest makes is queued, in order"
    );
}

#[test]
fn events_reach_the_guest() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    plugin
        .on_event(Event::OutputMatched(MatchEvent {
            pane: 7,
            pattern: "hello.greeting".to_string(),
            line: "a line mentioning hello".to_string(),
            start: 17,
            end: 22,
        }))
        .expect("event delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin matched hello.greeting in pane 7".to_string(),
        }]
    );
}

#[test]
fn the_host_queues_only_what_the_guest_actually_asked_for() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    plugin
        .on_event(Event::CwdChanged(CwdEvent {
            pane: 1,
            path: "/tmp".to_string(),
        }))
        .expect("delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::SetStatus {
            id: "hello.counter".to_string(),
            text: "cwd: /tmp".to_string(),
        }],
        "this event only sets status, so no notification may appear"
    );
}

#[test]
fn an_ungranted_capability_drops_the_request_it_would_have_made() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    plugin
        .on_event(Event::MatchActivated(MatchEvent {
            pane: 1,
            pattern: "hello.issue".to_string(),
            line: "see #42 for details".to_string(),
            start: 4,
            end: 7,
        }))
        .expect("delivered");

    assert!(
        plugin.drain_requests().is_empty(),
        "open-url needs consent, so the guest's call must be dropped"
    );
}

#[test]
fn a_consented_capability_lets_the_request_through() {
    let Some(mut plugin) =
        load(&|info: &PluginInfo| grant_with_consent(info, &[Capability::OpenUrl]))
    else {
        return;
    };

    plugin
        .on_event(Event::MatchActivated(MatchEvent {
            pane: 1,
            pattern: "hello.issue".to_string(),
            line: "see #42 for details".to_string(),
            start: 4,
            end: 7,
        }))
        .expect("delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::OpenUrl {
            url: "https://example.com/issues/42".to_string(),
        }],
        "the guest receives the clicked span, not just the pattern id"
    );
}

fn greedy() -> PluginInfo {
    PluginInfo {
        name: "greedy".to_string(),
        version: "0.1.0".to_string(),
        description: None,
        author: None,
        homepage: None,
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
        auto(&greedy()),
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
    assert!(!auto(&greedy()).contains(&Capability::WritePty));
    assert!(!auto(&greedy()).contains(&Capability::Network));
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
        description: None,
        author: None,
        homepage: None,
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
    let Some(_plugin) = load_in(&root, &nothing) else {
        return;
    };

    assert!(!root.0.exists(), "nothing should be preopened");
}

#[test]
fn a_panicking_command_retires_the_plugin() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    assert!(matches!(
        plugin.run_command("hello.boom"),
        Err(PluginError::Trapped(_))
    ));
    assert!(
        plugin.failure().is_some(),
        "a trapped instance must be marked retired"
    );

    assert!(
        matches!(plugin.run_command("hello.hi"), Err(PluginError::Retired(_))),
        "later calls should report the original failure, not a re-entry error"
    );
}

#[test]
fn a_reported_failure_leaves_the_plugin_usable() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    let err = plugin
        .run_command("hello.fail")
        .expect_err("the guest returns Err");
    assert!(
        matches!(err, PluginError::Reported(_)),
        "a returned error is not a trap: {err}"
    );
    assert!(
        plugin.failure().is_none(),
        "a reported failure must not retire the instance"
    );

    plugin
        .run_command("hello.hi")
        .expect("the plugin still works after reporting a failure");
    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello from the hello plugin!".to_string(),
        }]
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
            settings: Default::default(),
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
            settings: Default::default(),
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    assert_eq!(registry.status("alpha"), Some(Status::Ready));
    assert!(
        registry
            .get_mut("alpha")
            .expect("ready")
            .granted()
            .contains(&Capability::Network),
        "consent recorded in config must reach the instance at startup"
    );
}

#[test]
fn without_recorded_consent_the_capability_stays_ungranted() {
    let root = TempRoot::new("cfg-no-consent");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(
        !registry
            .get_mut("alpha")
            .expect("ready")
            .granted()
            .contains(&Capability::Network),
        "a requested capability must stay off until the user approves it"
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
            settings: Default::default(),
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

#[test]
fn disabling_gives_the_plugin_a_chance_to_flush() {
    let root = TempRoot::new("shutdown");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.get_mut("alpha").expect("ready").drain_requests();

    registry.disable("alpha");

    assert_eq!(registry.status("alpha"), Some(Status::Disabled));
}

#[test]
fn shutdown_reaches_the_guest() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };
    plugin.drain_requests();

    plugin.shutdown().expect("shutdown runs");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin shutting down".to_string(),
        }]
    );
}

#[test]
fn a_trapped_plugin_is_not_asked_to_shut_down() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };
    plugin.run_command("hello.boom").expect_err("panics");
    plugin.drain_requests();

    plugin
        .shutdown()
        .expect("shutdown is skipped, not an error, once the instance is dead");

    assert!(
        plugin.drain_requests().is_empty(),
        "a trapped instance cannot be re-entered, so nothing should reach the guest"
    );
}

#[test]
fn a_declared_pattern_reaches_the_guest_as_a_match() {
    let root = TempRoot::new("match");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.get_mut("alpha").expect("ready").drain_requests();

    assert!(
        registry.watches_output(),
        "hello declares a pattern, so output capture must switch on"
    );

    let now = std::time::Instant::now();
    let events = registry.match_output(3, "a line saying hello there", now);

    assert_eq!(events.len(), 1);
    let (id, matched) = &events[0];
    assert_eq!(id, "alpha");
    assert_eq!(matched.pattern, "hello.greeting");
    assert_eq!(matched.pane, 3);
    assert_eq!(
        &matched.line[matched.start as usize..matched.end as usize],
        "hello"
    );

    let plugin = registry.get_mut("alpha").expect("ready");
    plugin
        .on_event(Event::OutputMatched(matched.clone()))
        .expect("delivered");
    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin matched hello.greeting in pane 3".to_string(),
        }]
    );
}

#[test]
fn a_line_matching_nothing_produces_no_events() {
    let root = TempRoot::new("nomatch");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let events = registry.match_output(1, "nothing of interest", std::time::Instant::now());

    assert!(events.is_empty());
}

#[test]
fn a_plugin_without_patterns_does_not_switch_capture_on() {
    let root = TempRoot::new("nopattern");
    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(
        !registry.watches_output(),
        "with no plugins there is nothing to match, so panes must not buffer output"
    );
}

#[test]
fn a_declared_setting_falls_back_to_its_default() {
    let root = TempRoot::new("setting-default");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let fields = registry.setting_fields("alpha");
    assert_eq!(fields.len(), 2);
    assert_eq!(
        registry.setting_value("alpha", "greeting").as_deref(),
        Some("hello"),
        "an unset field reads back as its declared default"
    );
}

#[test]
fn a_stored_setting_wins_over_the_default() {
    let root = TempRoot::new("setting-stored");
    if !install(&root, "alpha") {
        return;
    }
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: true,
            consented: Vec::new(),
            settings: [("greeting".to_string(), "howdy".to_string())]
                .into_iter()
                .collect(),
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    assert_eq!(
        registry.setting_value("alpha", "greeting").as_deref(),
        Some("howdy")
    );
}

#[test]
fn changing_a_setting_is_recorded_and_announced() {
    let root = TempRoot::new("setting-change");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let event = registry
        .set_setting("alpha", "greeting", "howdy".to_string())
        .expect("a real change produces an event");
    assert_eq!(event.key, "greeting");
    assert_eq!(event.value, "howdy");

    assert_eq!(
        registry
            .settings()
            .get("alpha")
            .and_then(|s| s.settings.get("greeting"))
            .map(String::as_str),
        Some("howdy"),
        "the value must be written back for persistence"
    );
}

#[test]
fn setting_a_value_to_what_it_already_is_announces_nothing() {
    let root = TempRoot::new("setting-noop");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(
        registry
            .set_setting("alpha", "greeting", "hello".to_string())
            .is_none(),
        "writing the current value must not wake the plugin"
    );
}

#[test]
fn a_setting_change_reaches_the_guest() {
    let root = TempRoot::new("setting-event");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    let changed = registry
        .set_setting("alpha", "greeting", "howdy".to_string())
        .expect("change");

    let plugin = registry.get_mut("alpha").expect("ready");
    plugin.drain_requests();
    plugin
        .on_event(Event::SettingChanged(changed))
        .expect("delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin saw greeting change to howdy".to_string(),
        }]
    );
}

#[test]
fn a_stored_setting_is_visible_to_read_config() {
    let root = TempRoot::new("setting-readconfig");
    if !install(&root, "alpha") {
        return;
    }
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: true,
            consented: vec!["read-config".to_string()],
            settings: [("greeting".to_string(), "howdy".to_string())]
                .into_iter()
                .collect(),
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    let plugin = registry.get_mut("alpha").expect("ready");
    plugin.drain_requests();
    plugin
        .run_command("hello.readconfig")
        .expect("command runs");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin read greeting=howdy".to_string(),
        }],
        "the guest must see the stored value through read-config"
    );
}

#[test]
fn a_disabled_plugin_still_reports_what_it_is() {
    let root = TempRoot::new("disabled-metadata");
    if !install(&root, "alpha") {
        return;
    }
    let mut settings = crate::config::plugins::PluginsConfig::new();
    settings.insert(
        "alpha".to_string(),
        crate::config::plugins::PluginSettings {
            enabled: false,
            consented: Vec::new(),
            settings: Default::default(),
        },
    );

    let mut registry = registry_with(&root, settings);
    registry.load_all();

    assert_eq!(registry.status("alpha"), Some(Status::Disabled));
    assert!(!registry.is_enabled("alpha"));
    assert_eq!(
        registry.info("alpha").map(|info| info.name.as_str()),
        Some("hello"),
        "the panel needs a name and version even while the plugin is off"
    );
    assert_eq!(
        registry.setting_fields("alpha").len(),
        2,
        "declared settings stay editable while the plugin is off"
    );
}

#[test]
fn disabling_keeps_the_declared_settings_visible() {
    let root = TempRoot::new("disable-keeps-fields");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    let before = registry.setting_fields("alpha").len();
    registry.disable("alpha");

    assert_eq!(
        registry.setting_fields("alpha").len(),
        before,
        "turning a plugin off must not empty its settings panel"
    );
}

#[test]
fn granted_reflects_consent_without_asking_the_instance() {
    let root = TempRoot::new("granted-view");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(
        !registry.granted("alpha").contains(&Capability::Network),
        "a consent-gated capability starts ungranted"
    );

    registry.consent("alpha", Capability::Network);
    assert!(
        registry.granted("alpha").contains(&Capability::Network),
        "consent must show up in the panel before the restart"
    );

    registry.revoke("alpha", Capability::Network);
    assert!(!registry.granted("alpha").contains(&Capability::Network));
    assert!(
        registry
            .settings()
            .get("alpha")
            .is_some_and(|settings| settings.consented.is_empty()),
        "revoking must be written back so it survives a restart"
    );
}

#[test]
fn consent_takes_effect_after_the_plugin_restarts() {
    let root = TempRoot::new("consent-restart");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.consent("alpha", Capability::Network);
    registry.enable("alpha").expect("restart");

    let plugin = registry.get_mut("alpha").expect("ready");
    assert!(
        plugin.granted().contains(&Capability::Network),
        "capabilities are fixed at instantiation, so consent needs a restart to reach the guest"
    );
}

#[test]
fn contributed_commands_carry_a_title_and_their_source() {
    let root = TempRoot::new("contributed");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let commands = registry.contributed_commands();
    let hi = commands
        .iter()
        .find(|command| command.id == "hello.hi")
        .expect("hello.hi is declared");

    assert_eq!(hi.plugin, "alpha", "the owning entry, used to dispatch");
    assert_eq!(hi.source, "hello", "the display name, shown to the user");
    assert_eq!(
        hi.title, "Say hi",
        "the palette shows the title, not the id"
    );
}

#[test]
fn a_disabled_plugin_contributes_no_commands() {
    let root = TempRoot::new("contributed-disabled");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    assert!(!registry.contributed_commands().is_empty());

    registry.disable("alpha");
    assert!(
        registry.contributed_commands().is_empty(),
        "a disabled plugin must not be runnable from the palette"
    );
}

#[test]
fn a_retired_plugin_contributes_no_commands() {
    let root = TempRoot::new("contributed-retired");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let plugin = registry.get_mut("alpha").expect("ready");
    let _ = plugin.run_command("hello.boom");
    registry.retire_failed();

    assert!(
        registry.contributed_commands().is_empty(),
        "a trapped plugin must drop out of the palette"
    );
}

#[test]
fn contributed_menu_items_are_split_by_context() {
    let root = TempRoot::new("menu-context");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let terminal = registry.menu_items(MenuContext::Terminal);
    let tab = registry.menu_items(MenuContext::Tab);

    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0].1.id, "hello.hi");
    assert_eq!(tab.len(), 1);
    assert_eq!(tab[0].1.id, "hello.readconfig");
}

#[test]
fn a_disabled_plugin_contributes_no_menu_items() {
    let root = TempRoot::new("menu-disabled");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    registry.disable("alpha");

    assert!(registry.menu_items(MenuContext::Terminal).is_empty());
}

#[test]
fn an_activated_menu_item_carries_the_selection() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    plugin
        .on_event(Event::MenuActivated(MenuEvent {
            item: "hello.hi".to_string(),
            pane: 3,
            selection: Some("selected text".to_string()),
        }))
        .expect("delivered");

    assert_eq!(
        plugin.drain_requests(),
        vec![PluginRequest::Notify {
            message: "hello plugin menu hello.hi in pane 3 over selected text".to_string(),
        }]
    );
}

#[test]
fn a_declared_status_slot_can_be_updated_but_an_undeclared_one_cannot() {
    let root = TempRoot::new("status-slot");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    assert!(
        registry.set_status("alpha", "hello.counter", "changed".to_string()),
        "a declared slot accepts updates"
    );
    assert!(
        !registry.set_status("alpha", "hello.nope", "changed".to_string()),
        "an undeclared slot must be refused, not silently created"
    );
    assert_eq!(
        registry
            .status_items()
            .iter()
            .find(|(_, item)| item.id == "hello.counter")
            .map(|(_, item)| item.text.as_str()),
        Some("changed")
    );
}

#[test]
fn a_command_can_declare_a_default_key() {
    let root = TempRoot::new("default-key");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let commands = registry.contributed_commands();
    let hi = commands
        .iter()
        .find(|command| command.id == "hello.hi")
        .expect("hello.hi");

    assert_eq!(hi.default_key.as_deref(), Some("Ctrl+Shift+H"));
    assert!(
        commands
            .iter()
            .find(|command| command.id == "hello.fail")
            .is_some_and(|command| command.default_key.is_none()),
        "a command without a key must stay unbound"
    );
}

#[test]
fn a_plugin_can_supply_profiles() {
    let root = TempRoot::new("profiles");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let profiles = registry
        .get_mut("alpha")
        .expect("ready")
        .list_profiles()
        .expect("listed");

    assert_eq!(profiles.len(), 2);
    assert!(matches!(profiles[0].target, ProfileTarget::Local(_)));
    match &profiles[1].target {
        ProfileTarget::Ssh(target) => {
            assert_eq!(target.host, "example.invalid");
            assert_eq!(target.port, 22);
        }
        other => panic!("expected an ssh target, got {other:?}"),
    }
}

#[test]
fn declared_profiles_are_read_back_from_the_cache() {
    let root = TempRoot::new("profile-cache");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let profiles = registry.profiles();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0].0, "alpha", "each profile carries its owner");
    assert_eq!(profiles[0].1.name, "Hello echo");

    let names: Vec<String> = registry
        .profiles()
        .into_iter()
        .map(|(_, profile)| profile.name)
        .collect();
    assert_eq!(names, vec!["Hello echo", "Hello SSH"]);
}

#[test]
fn a_disabled_plugin_supplies_no_profiles() {
    let root = TempRoot::new("profile-disabled");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    assert!(!registry.profiles().is_empty());

    registry.disable("alpha");
    assert!(
        registry.profiles().is_empty(),
        "a disabled plugin must not keep offering profiles"
    );
}

#[test]
fn a_retired_plugin_supplies_no_profiles() {
    let root = TempRoot::new("profile-retired");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let plugin = registry.get_mut("alpha").expect("ready");
    let _ = plugin.run_command("hello.boom");
    registry.retire_failed();

    assert!(registry.profiles().is_empty());
}

#[test]
fn the_manifest_carries_authorship() {
    let Some(plugin) = load(&auto) else {
        return;
    };

    let info = plugin.info();
    assert_eq!(info.author.as_deref(), Some("Rabbitty"));
    assert_eq!(
        info.homepage.as_deref(),
        Some("https://github.com/wHoIsDReAmer/RabbiTTY")
    );
    assert!(info.description.is_some());
}

#[test]
fn a_disabled_plugin_leaves_no_status_behind() {
    let root = TempRoot::new("status-disabled");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();
    assert!(!registry.status_items().is_empty());

    registry.disable("alpha");
    assert!(
        registry.status_items().is_empty(),
        "a stopped plugin must not keep occupying the status bar"
    );
}

#[test]
fn a_retired_plugin_leaves_no_status_behind() {
    let root = TempRoot::new("status-retired");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let plugin = registry.get_mut("alpha").expect("ready");
    let _ = plugin.run_command("hello.boom");
    registry.retire_failed();

    assert!(
        registry.status_items().is_empty(),
        "a crashed plugin must not leave stale text on screen"
    );
}

#[test]
fn a_status_update_from_an_event_reaches_the_registry() {
    let root = TempRoot::new("status-event");
    if !install(&root, "alpha") {
        return;
    }

    let mut registry = registry_in(&root);
    registry.load_all();

    let plugin = registry.get_mut("alpha").expect("ready");
    plugin.drain_requests();
    plugin
        .on_event(Event::SessionStart(9))
        .expect("event delivered");
    let requests = plugin.drain_requests();

    let update = requests
        .iter()
        .find_map(|request| match request {
            PluginRequest::SetStatus { id, text } => Some((id.clone(), text.clone())),
            _ => None,
        })
        .expect("the guest asked for a status update");
    assert_eq!(
        update,
        ("hello.counter".to_string(), "hello: pane 9".to_string())
    );

    assert!(registry.set_status("alpha", &update.0, update.1.clone()));
    assert_eq!(
        registry
            .status_items()
            .into_iter()
            .find(|(_, item)| item.id == "hello.counter")
            .map(|(_, item)| item.text),
        Some(update.1)
    );
}

#[test]
fn a_memory_hungry_plugin_is_stopped_instead_of_growing_without_bound() {
    let Some(mut plugin) = load(&auto) else {
        return;
    };

    let outcome = plugin.run_command("hello.hog");

    assert!(
        matches!(outcome, Err(PluginError::Trapped(_))),
        "the store limit must stop the allocation, got {outcome:?}"
    );
    assert!(
        plugin.failure().is_some(),
        "a plugin that hit the memory ceiling is retired like any other trap"
    );
    assert!(
        plugin.drain_requests().is_empty(),
        "it never reached its notify call"
    );
}
