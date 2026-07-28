use super::super::App;
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
