use super::super::App;
use crate::gui::settings::SettingsCategory;
use crate::gui::settings::plugins::{PluginPermission, PluginState};
use crate::plugin::{Event, PluginRequest};

impl App {
    pub(in crate::gui) fn dispatch_plugin_event(&mut self, event: Event) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };

        let mut requests: Vec<(String, PluginRequest)> = Vec::new();
        let mut reported: Vec<(String, String)> = Vec::new();
        for (id, plugin) in registry.ready_mut() {
            if let Err(crate::plugin::PluginError::Reported(reason)) =
                plugin.on_event(event.clone())
            {
                reported.push((id.to_string(), reason));
            }
            requests.extend(
                plugin
                    .drain_requests()
                    .into_iter()
                    .map(|request| (id.to_string(), request)),
            );
        }

        for (id, reason) in reported {
            eprintln!("plugin {id} reported an error handling an event: {reason}");
        }

        for (id, reason) in registry.retire_failed() {
            eprintln!("plugin {id} retired: {reason}");
        }

        for (source, request) in requests {
            self.apply_plugin_request(&source, request);
        }
    }

    fn dispatch_to_plugin(&mut self, id: &str, event: Event) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        let Some(plugin) = registry.get_mut(id) else {
            return;
        };

        let outcome = plugin.on_event(event);
        let requests = plugin.drain_requests();

        if let Err(crate::plugin::PluginError::Reported(reason)) = outcome {
            eprintln!("plugin {id} reported an error handling an event: {reason}");
        }
        for (id, reason) in registry.retire_failed() {
            eprintln!("plugin {id} retired: {reason}");
        }
        for request in requests {
            self.apply_plugin_request(id, request);
        }
    }

    pub(in crate::gui) fn run_plugin_command(&mut self, plugin: &str, command: &str) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        let Some(instance) = registry.get_mut(plugin) else {
            return;
        };

        let outcome = instance.run_command(command);
        let requests = instance.drain_requests();
        let retired = registry.retire_failed();

        if let Err(crate::plugin::PluginError::Reported(reason)) = outcome {
            self.notify_from(plugin, &reason);
        }
        for (id, reason) in retired {
            eprintln!("plugin {id} retired: {reason}");
        }
        for request in requests {
            self.apply_plugin_request(plugin, request);
        }
    }

    fn notify_from(&self, source: &str, message: &str) {
        let name = self
            .plugins
            .as_ref()
            .and_then(|registry| registry.info(source))
            .map(|info| info.name.as_str())
            .unwrap_or(source);
        crate::platform::notify(name, message);
    }

    fn apply_plugin_request(&mut self, source: &str, request: PluginRequest) {
        match request {
            PluginRequest::WritePty { pane, data } => {
                if let Some(target) = self.pane_mut_by_id(pane)
                    && !target.send_bytes(&data)
                {
                    eprintln!("plugin write to pane {pane} failed");
                }
            }
            PluginRequest::Notify { message } => {
                self.notify_from(source, &message);
            }
        }
    }

    pub(in crate::gui) fn match_output_lines(&mut self, pane: u64, lines: Vec<String>) {
        if lines.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        let mut events = Vec::new();
        if let Some(registry) = self.plugins.as_mut() {
            for line in &lines {
                events.extend(registry.match_output(pane, line, now));
            }
        }
        for (id, matched) in events {
            self.dispatch_to_plugin(&id, crate::plugin::Event::OutputMatched(matched));
        }
    }

    pub(in crate::gui) fn sync_output_capture(&mut self) {
        let watching = self
            .plugins
            .as_ref()
            .is_some_and(|registry| registry.watches_output());
        for pane in self.panes_mut() {
            pane.capture_output = watching;
        }
    }

    pub(in crate::gui) fn settings_categories(&self) -> Vec<(SettingsCategory, String)> {
        let mut out: Vec<(SettingsCategory, String)> = SettingsCategory::BUILTIN
            .into_iter()
            .map(|category| (category, category.label().to_string()))
            .collect();
        if let Some(registry) = self.plugins.as_ref() {
            let ids: Vec<String> = registry.ids().map(|id| id.to_string()).collect();
            for (index, id) in ids.into_iter().enumerate() {
                let label = registry
                    .info(&id)
                    .map(|info| info.name.clone())
                    .unwrap_or(id);
                out.push((SettingsCategory::Plugin(index), label));
            }
        }
        out
    }

    pub(in crate::gui) fn plugin_id_at(&self, index: usize) -> Option<String> {
        self.plugins
            .as_ref()?
            .ids()
            .nth(index)
            .map(|id| id.to_string())
    }

    pub(in crate::gui) fn refresh_plugin_settings(&mut self) {
        let SettingsCategory::Plugin(index) = self.settings_category else {
            return;
        };
        let Some(id) = self.plugin_id_at(index) else {
            self.plugin_settings = Default::default();
            return;
        };
        let Some(registry) = self.plugins.as_ref() else {
            return;
        };

        let (state, failure) = match registry.status(&id) {
            Some(crate::plugin::Status::Ready) => (PluginState::Ready, None),
            Some(crate::plugin::Status::Disabled) => (PluginState::Disabled, None),
            Some(crate::plugin::Status::Retired(reason)) => (PluginState::Retired, Some(reason)),
            None => (PluginState::Disabled, None),
        };

        let info = registry.info(&id);
        let granted = registry.granted(&id);
        let permissions = info
            .map(|info| {
                let consent = crate::plugin::requires_consent(info);
                info.capabilities
                    .iter()
                    .map(|cap| PluginPermission {
                        name: crate::plugin::capability_name(*cap).to_string(),
                        granted: granted.contains(cap),
                        optional: consent.contains(cap),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let fields = registry
            .setting_fields(&id)
            .into_iter()
            .map(|field| {
                let value = registry
                    .setting_value(&id, &field.key)
                    .unwrap_or_else(|| field.default_value.clone());
                (field, value)
            })
            .collect();

        self.plugin_settings = crate::gui::settings::plugins::PluginSettingsState {
            name: info
                .map(|info| info.name.clone())
                .unwrap_or_else(|| id.clone()),
            version: info.map(|info| info.version.clone()).unwrap_or_default(),
            enabled: registry.is_enabled(&id),
            state,
            failure,
            permissions,
            fields,
            id,
        };
    }

    pub(in crate::gui) fn toggle_plugin(&mut self, plugin: &str, enabled: bool) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        if enabled {
            if let Err(reason) = registry.enable(plugin) {
                eprintln!("plugin {plugin} failed to start: {reason}");
            }
        } else {
            registry.disable(plugin);
        }
        self.persist_plugin_settings();
        self.sync_output_capture();
        self.refresh_plugin_settings();
    }

    pub(in crate::gui) fn change_plugin_consent(
        &mut self,
        plugin: &str,
        capability: &str,
        granted: bool,
    ) {
        let Some(capability) = crate::plugin::capability_from_name(capability) else {
            return;
        };
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        if granted {
            registry.consent(plugin, capability);
        } else {
            registry.revoke(plugin, capability);
        }
        let restart = registry.is_enabled(plugin);
        self.persist_plugin_settings();
        if restart {
            self.reload_plugin(plugin);
        } else {
            self.refresh_plugin_settings();
        }
    }

    pub(in crate::gui) fn reload_plugin(&mut self, plugin: &str) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        if let Err(reason) = registry.enable(plugin) {
            eprintln!("plugin {plugin} failed to start: {reason}");
        }
        self.persist_plugin_settings();
        self.sync_output_capture();
        self.refresh_plugin_settings();
    }

    fn persist_plugin_settings(&mut self) {
        let Some(registry) = self.plugins.as_ref() else {
            return;
        };
        self.config.plugins = registry.settings().clone();
        self.queue_config_save();
    }

    pub(in crate::gui) fn change_plugin_setting(&mut self, plugin: &str, key: &str, value: String) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        let Some(changed) = registry.set_setting(plugin, key, value) else {
            return;
        };
        self.persist_plugin_settings();
        self.dispatch_to_plugin(plugin, Event::SettingChanged(changed));
        self.refresh_plugin_settings();
    }

    pub(in crate::gui) fn shutdown_plugins(&mut self) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        for (id, reason) in registry.shutdown_all() {
            eprintln!("plugin {id} failed to shut down cleanly: {reason}");
        }
        if self.config.plugins != *registry.settings() {
            self.config.plugins = registry.settings().clone();
            if let Err(err) = self.config.save() {
                eprintln!("failed to persist plugin settings: {err}");
            }
        }
    }
}
