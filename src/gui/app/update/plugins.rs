use super::super::App;
use crate::plugin::{Event, PluginRequest};

impl App {
    pub(in crate::gui) fn dispatch_plugin_event(&mut self, event: Event) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };

        let mut requests: Vec<PluginRequest> = Vec::new();
        for (_, plugin) in registry.ready_mut() {
            let _ = plugin.on_event(event.clone());
            requests.append(&mut plugin.drain_requests());
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

    pub(in crate::gui) fn shutdown_plugins(&mut self) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        registry.shutdown_all();
        if self.config.plugins != *registry.settings() {
            self.config.plugins = registry.settings().clone();
            if let Err(err) = self.config.save() {
                eprintln!("failed to persist plugin settings: {err}");
            }
        }
    }
}
