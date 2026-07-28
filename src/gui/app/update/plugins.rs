use super::super::App;
use crate::gui::settings::SettingsCategory;
use crate::plugin::{Event, PluginRequest};

impl App {
    pub(in crate::gui) fn dispatch_plugin_event(&mut self, event: Event) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };

        let mut requests: Vec<PluginRequest> = Vec::new();
        let mut reported: Vec<(String, String)> = Vec::new();
        for (id, plugin) in registry.ready_mut() {
            if let Err(crate::plugin::PluginError::Reported(reason)) =
                plugin.on_event(event.clone())
            {
                reported.push((id.to_string(), reason));
            }
            requests.append(&mut plugin.drain_requests());
        }

        for (id, reason) in reported {
            eprintln!("plugin {id} reported an error handling an event: {reason}");
        }

        for (id, reason) in registry.retire_failed() {
            eprintln!("plugin {id} retired: {reason}");
        }

        for request in requests {
            self.apply_plugin_request(request);
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
            self.apply_plugin_request(request);
        }
    }

    fn apply_plugin_request(&mut self, request: PluginRequest) {
        match request {
            PluginRequest::WritePty { pane, data } => {
                if let Some(target) = self.pane_mut_by_id(pane)
                    && !target.send_bytes(&data)
                {
                    eprintln!("plugin write to pane {pane} failed");
                }
            }
            PluginRequest::Notify { message } => {
                eprintln!("plugin notification: {message}");
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
            for (index, id) in registry.ids().enumerate() {
                out.push((SettingsCategory::Plugin(index), id.to_string()));
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

        let status = match registry.status(&id) {
            Some(crate::plugin::Status::Ready) => crate::t!("settings.plugins.ready").to_string(),
            Some(crate::plugin::Status::Disabled) => {
                crate::t!("settings.plugins.disabled").to_string()
            }
            Some(crate::plugin::Status::Retired(reason)) => reason,
            None => String::new(),
        };
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

        self.plugin_settings =
            crate::gui::settings::plugins::PluginSettingsState { id, status, fields };
    }

    pub(in crate::gui) fn change_plugin_setting(&mut self, plugin: &str, key: &str, value: String) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        let Some(changed) = registry.set_setting(plugin, key, value) else {
            return;
        };
        self.config.plugins = registry.settings().clone();
        self.queue_config_save();
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
