use super::super::{App, Message};
use crate::gui::settings::SettingsCategory;
use crate::gui::settings::plugins::{PluginPermission, PluginState};
use crate::plugin::{Event, PluginRequest};
use iced::Task;

const PROFILE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

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

    pub(in crate::gui) fn dispatch_to_plugin(&mut self, id: &str, event: Event) {
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

    pub(in crate::gui) fn adopt_plugin_shortcuts(&mut self) {
        let Some(registry) = self.plugins.as_ref() else {
            return;
        };
        let wanted: Vec<(String, String, String)> = registry
            .contributed_commands()
            .into_iter()
            .filter_map(|command| {
                let key = command.default_key?;
                Some((command.plugin, command.id, key))
            })
            .collect();

        for (plugin, command, key) in wanted {
            if self
                .config
                .shortcuts
                .plugin_binding(&plugin, &command)
                .is_some()
            {
                continue;
            }
            if self.config.shortcuts.is_taken(&key) {
                eprintln!(
                    "plugin {plugin} wanted {key} for {command}, but it is already bound; \
                     leaving the command unbound"
                );
                continue;
            }
            self.config
                .shortcuts
                .set_plugin_binding(&plugin, &command, key);
        }
    }

    pub(in crate::gui) fn sync_plugin_shortcut_draft(&mut self) {
        let Some(registry) = self.plugins.as_ref() else {
            self.settings_draft.plugin_shortcuts.clear();
            return;
        };
        let rows: Vec<crate::gui::settings::PluginShortcutDraft> = registry
            .contributed_commands()
            .into_iter()
            .map(|command| crate::gui::settings::PluginShortcutDraft {
                binding: self
                    .config
                    .shortcuts
                    .plugin_binding(&command.plugin, &command.id)
                    .unwrap_or_default()
                    .to_string(),
                label: format!("{} — {}", command.source, command.title),
                plugin: command.plugin,
                command: command.id,
            })
            .collect();
        self.settings_draft.plugin_shortcuts = rows;
    }

    pub(in crate::gui) fn commit_plugin_shortcut(&mut self, index: usize) {
        let Some(row) = self.settings_draft.plugin_shortcuts.get(index).cloned() else {
            return;
        };
        let binding = row.binding.trim().to_string();
        let current = self
            .config
            .shortcuts
            .plugin_binding(&row.plugin, &row.command)
            .unwrap_or_default();
        if binding == current {
            return;
        }
        if !binding.is_empty() && self.config.shortcuts.is_taken(&binding) {
            self.sync_plugin_shortcut_draft();
            return;
        }
        self.config
            .shortcuts
            .set_plugin_binding(&row.plugin, &row.command, binding);
        self.queue_config_save();
        self.sync_plugin_shortcut_draft();
    }

    pub(in crate::gui) fn resolve_plugin_shortcut(
        &self,
        physical: &iced::keyboard::key::Physical,
        modifiers: iced::keyboard::Modifiers,
    ) -> Option<(String, String)> {
        self.config
            .shortcuts
            .plugin_iter()
            .find(|(_, binding)| {
                crate::gui::app::shortcuts::shortcut_matches(binding, physical, modifiers)
            })
            .and_then(|(key, _)| crate::config::split_plugin_key(key))
            .map(|(plugin, command)| (plugin.to_string(), command.to_string()))
    }

    pub(in crate::gui) fn plugin_picker_profiles(
        &self,
    ) -> Vec<(String, Option<String>, crate::gui::tab::Profile)> {
        let Some(registry) = self.plugins.as_ref() else {
            return Vec::new();
        };
        registry
            .profiles()
            .into_iter()
            .map(|(_, declared)| {
                let subtitle = declared.subtitle.clone();
                (declared.name.clone(), subtitle, into_profile(declared))
            })
            .collect()
    }

    pub(in crate::gui) fn plugin_menu_items(
        &self,
        context: crate::plugin::MenuContext,
    ) -> Vec<crate::gui::components::context_menu::ContextMenuItem> {
        let Some(registry) = self.plugins.as_ref() else {
            return Vec::new();
        };
        registry
            .menu_items(context)
            .into_iter()
            .map(
                |(plugin, item)| crate::gui::components::context_menu::ContextMenuItem {
                    label: item.title,
                    message: Message::RunPluginMenuItem {
                        plugin,
                        item: item.id,
                    },
                },
            )
            .collect()
    }

    pub(in crate::gui) fn activate_plugin_menu_item(&mut self, plugin: &str, item: &str) {
        let pane = self.focused_pane().map(|pane| pane.id).unwrap_or_default();
        let selection = self.focused_pane().and_then(|pane| pane.selected_text());
        self.dispatch_to_plugin(
            plugin,
            Event::MenuActivated(crate::plugin::MenuEvent {
                item: item.to_string(),
                pane,
                selection,
            }),
        );
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
            PluginRequest::OpenUrl { url } => crate::platform::open_url(&url),
            PluginRequest::SetStatus { id, text } => {
                if let Some(registry) = self.plugins.as_mut()
                    && !registry.set_status(source, &id, text)
                {
                    eprintln!("plugin {source} set an undeclared status item: {id}");
                }
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
        let listening = self
            .plugins
            .as_ref()
            .is_some_and(|registry| registry.has_ready());
        for pane in self.panes_mut() {
            pane.capture_output = watching;
            pane.track_cwd = listening;
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

    pub(in crate::gui) fn plugins_overview(&self) -> crate::gui::settings::PluginsOverview {
        let Some(registry) = self.plugins.as_ref() else {
            return crate::gui::settings::PluginsOverview::default();
        };
        let ids: Vec<&str> = registry.ids().collect();
        let disabled = ids.iter().filter(|id| !registry.is_enabled(id)).count();
        crate::gui::settings::PluginsOverview {
            root: registry.root().to_path_buf(),
            installed: ids.len(),
            disabled,
            notice: self.plugin_notice.clone(),
        }
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
            description: info.and_then(|info| info.description.clone()),
            author: info.and_then(|info| info.author.clone()),
            homepage: info
                .and_then(|info| info.homepage.clone())
                .filter(|url| crate::terminal::url::is_openable(url)),
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
        self.adopt_plugin_shortcuts();
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
        self.adopt_plugin_shortcuts();
        self.sync_output_capture();
        self.refresh_plugin_settings();
    }

    /// A plugin that enumerates a network inventory can block for as long as its
    /// source takes, and fuel only bounds instructions, not blocking I/O. So the
    /// call happens off the UI thread and gives up after a deadline.
    pub(in crate::gui) fn refresh_plugin_profiles(&self) -> Task<Message> {
        let Some(registry) = self.plugins.as_ref() else {
            return Task::none();
        };
        let host = registry.host();
        let fetches: Vec<Task<Message>> = registry
            .profile_sources()
            .into_iter()
            .map(|source| {
                let host = std::sync::Arc::clone(&host);
                let plugin = source.id.clone();
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            crate::plugin::fetch_profiles_with_deadline(
                                &host,
                                &source,
                                PROFILE_DEADLINE,
                            )
                        })
                        .await
                        .unwrap_or(None)
                    },
                    move |profiles| Message::PluginProfilesFetched {
                        plugin: plugin.clone(),
                        profiles,
                    },
                )
            })
            .collect();
        Task::batch(fetches)
    }

    pub(in crate::gui) fn apply_fetched_profiles(
        &mut self,
        plugin: &str,
        profiles: Option<Vec<crate::plugin::PluginProfile>>,
    ) {
        let Some(profiles) = profiles else {
            return;
        };
        if let Some(registry) = self.plugins.as_mut() {
            registry.set_profiles(plugin, profiles);
        }
    }

    /// Reads the manifest before anything is copied, so the user sees what the
    /// plugin is and what it asks for while they can still decline.
    pub(in crate::gui) fn preview_plugin_install(&mut self, source: &std::path::Path) {
        let Some(registry) = self.plugins.as_ref() else {
            return;
        };
        match registry.preview(source) {
            Ok(info) => {
                self.plugin_notice = None;
                self.plugin_pending_install = Some(crate::gui::app::PendingInstall {
                    path: source.to_path_buf(),
                    summary: install_summary(&info),
                });
            }
            Err(reason) => {
                self.plugin_pending_install = None;
                self.plugin_notice = Some(reason);
            }
        }
    }

    pub(in crate::gui) fn install_pending_plugin(&mut self) {
        let Some(pending) = self.plugin_pending_install.take() else {
            return;
        };
        self.install_plugin(&pending.path);
    }

    pub(in crate::gui) fn install_plugin(&mut self, source: &std::path::Path) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        self.plugin_notice = match registry.install(source) {
            Ok(name) => {
                Some(crate::t!("settings.plugins.installed_notice").replace("{name}", &name))
            }
            Err(reason) => Some(reason),
        };
        self.after_registry_change();
    }

    pub(in crate::gui) fn remove_pending_plugin(&mut self) {
        let Some(id) = self.plugin_pending_removal.take() else {
            return;
        };
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        if let Err(reason) = registry.uninstall(&id) {
            self.plugin_notice = Some(reason);
        } else {
            self.plugin_notice = None;
            self.settings_category = SettingsCategory::Plugins;
        }
        self.after_registry_change();
    }

    fn after_registry_change(&mut self) {
        self.persist_plugin_settings();
        self.adopt_plugin_shortcuts();
        self.sync_output_capture();
        self.sync_plugin_shortcut_draft();
        self.refresh_plugin_settings();
    }

    pub(in crate::gui) fn rescan_plugins(&mut self) {
        let Some(registry) = self.plugins.as_mut() else {
            return;
        };
        registry.load_all();
        self.persist_plugin_settings();
        self.adopt_plugin_shortcuts();
        self.sync_output_capture();
        self.refresh_plugin_settings();
    }

    pub(in crate::gui) fn open_plugin_folder(&self) {
        if let Some(registry) = self.plugins.as_ref() {
            crate::platform::open_path(registry.root());
        }
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

fn into_profile(declared: crate::plugin::PluginProfile) -> crate::gui::tab::Profile {
    use crate::gui::tab::{Profile, ProfileKind};

    let kind = match declared.target {
        crate::plugin::ProfileTarget::Local(local) => ProfileKind::Local {
            program: local.program.filter(|program| !program.trim().is_empty()),
            args: local.args,
        },
        crate::plugin::ProfileTarget::Ssh(ssh) => ProfileKind::Ssh(crate::config::SshProfile {
            name: declared.name.clone(),
            host: ssh.host,
            port: ssh.port,
            user: ssh.user,
            auth_method: match ssh.identity_file {
                Some(_) => crate::config::SshAuthMethod::KeyFile,
                None => crate::config::SshAuthMethod::Password,
            },
            identity_file: ssh.identity_file,
            password: None,
            proxy_command: None,
        }),
    };

    Profile {
        name: declared.name,
        icon: declared.icon,
        kind,
    }
}

fn install_summary(info: &crate::plugin::PluginInfo) -> String {
    let mut lines = vec![format!("{} {}", info.name, info.version)];
    if let Some(author) = &info.author {
        lines.push(author.clone());
    }
    if let Some(description) = &info.description {
        lines.push(description.clone());
    }

    let wanted: Vec<&str> = info
        .capabilities
        .iter()
        .map(|cap| {
            crate::gui::settings::plugins::capability_label(crate::plugin::capability_name(*cap))
        })
        .collect();
    if !wanted.is_empty() {
        lines.push(format!(
            "{}: {}",
            crate::t!("settings.plugins.permissions"),
            wanted.join(", ")
        ));
    }
    lines.join("\n")
}
