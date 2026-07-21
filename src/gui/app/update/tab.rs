use super::super::shortcuts::ShortcutAction;
use super::super::{App, Message, SETTINGS_TAB_INDEX};
use crate::config::SshProfile;
use crate::gui::settings::SettingsDraft;
use crate::gui::tab::Profile;
use crate::terminal::TerminalTheme;
use iced::Task;
use iced::keyboard::{Key, Modifiers};

impl App {
    /// Request an SSH tab for `profile`. Defers tab creation through the
    /// password prompt when password auth is required but no credential is
    /// available yet.
    pub(in crate::gui) fn request_ssh_tab(&mut self, profile: SshProfile) -> Task<Message> {
        use crate::config::SshAuthMethod;
        if matches!(profile.auth_method, SshAuthMethod::Password)
            && profile.password.is_none()
            && crate::keychain::get_password(&profile.host, &profile.user).is_none()
        {
            self.password_prompt = Some(crate::gui::app::PasswordPromptState {
                profile,
                draft: String::new(),
                save_to_keychain: true,
                error: None,
            });
            return Task::none();
        }
        self.create_tab(Profile::ssh(profile))
    }

    /// Launch a tab for `profile`, deferring through the SSH password prompt
    /// when the profile is an SSH connection that needs one.
    pub(in crate::gui) fn launch_profile(&mut self, profile: Profile) -> Task<Message> {
        if let Some(ssh) = profile.ssh_profile() {
            let ssh = ssh.clone();
            self.request_ssh_tab(ssh)
        } else {
            self.create_tab(profile)
        }
    }

    pub(in crate::gui) fn create_tab(&mut self, profile: Profile) -> Task<Message> {
        let Some(sender) = self.pty_sender.clone() else {
            eprintln!("PTY output channel not ready");
            return Task::none();
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let display_name = profile.display_name();
        self.session_history.record(profile.clone(), display_name);
        let new_tab = crate::gui::tab::TerminalTab::from_profile(
            profile,
            cols,
            rows,
            theme,
            tab_id,
            sender,
            &self.config,
        );
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.dismiss_shell_picker();
        Task::none()
    }

    pub(super) fn handle_close_tab(&mut self, index: usize) {
        if index == SETTINGS_TAB_INDEX {
            self.settings_open = false;
            if self.active_tab == SETTINGS_TAB_INDEX {
                self.active_tab = self.tabs.len().saturating_sub(1);
            }
        } else if index < self.tabs.len() {
            self.tabs.remove(index);
            if self.active_tab != SETTINGS_TAB_INDEX {
                self.clamp_active_tab();
            }
        }
    }

    fn clamp_active_tab(&mut self) {
        if self.tabs.is_empty() {
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    pub(in crate::gui) fn shell_picker_option_count(&self) -> usize {
        self.session_ssh_profiles().len()
            + self.session_local_profiles().len()
            + self.available_shells.len()
    }

    pub(in crate::gui) fn session_ssh_profiles(&self) -> Vec<SshProfile> {
        let mut profiles: Vec<SshProfile> = if self.settings_open {
            let draft: Vec<SshProfile> = self
                .settings_draft
                .profiles
                .iter()
                .filter_map(|profile| profile.to_ssh_profile())
                .collect();
            if draft.is_empty() {
                self.config.ssh_profiles()
            } else {
                draft
            }
        } else {
            self.config.ssh_profiles()
        };

        // ~/.ssh/config-derived hosts join the list, but a user-created profile
        // with the same name wins.
        for cfg_profile in &self.ssh_config_profiles {
            if !profiles.iter().any(|p| p.name == cfg_profile.name) {
                profiles.push(cfg_profile.clone());
            }
        }
        profiles
    }

    pub(in crate::gui) fn session_local_profiles(&self) -> Vec<Profile> {
        let source = if self.settings_open {
            let draft: Vec<Profile> = self
                .settings_draft
                .profiles
                .iter()
                .filter(|d| !matches!(d.kind, crate::gui::settings::ProfileDraftKind::Ssh))
                .filter_map(|d| d.to_profile())
                .collect();
            if draft.is_empty() {
                self.config_local_profiles()
            } else {
                draft
            }
        } else {
            self.config_local_profiles()
        };
        source
            .into_iter()
            .filter(|p| !p.display_name().trim().is_empty())
            .collect()
    }

    fn config_local_profiles(&self) -> Vec<Profile> {
        self.config
            .profiles
            .iter()
            .filter(|p| p.ssh_profile().is_none())
            .cloned()
            .collect()
    }

    pub(super) fn shift_shell_picker_selection(&mut self, delta: isize) {
        let count = self.shell_picker_option_count() as isize;
        if count <= 0 {
            return;
        }

        let next = (self.shell_picker_selected as isize + delta).rem_euclid(count) as usize;
        self.shell_picker_selected = next;
    }

    pub(super) fn confirm_shell_picker_selection(&mut self) -> Task<Message> {
        let selected = self.shell_picker_selected;
        let ssh_profiles = self.session_ssh_profiles();
        let local_profiles = self.session_local_profiles();

        if selected < ssh_profiles.len() {
            let profile = ssh_profiles[selected].clone();
            self.dismiss_shell_picker();
            return self.request_ssh_tab(profile);
        }

        let local_index = selected - ssh_profiles.len();
        if local_index < local_profiles.len() {
            let profile = local_profiles[local_index].clone();
            return self.create_tab(profile);
        }

        let shell_index = local_index - local_profiles.len();
        if shell_index < self.available_shells.len() {
            let shell = self.available_shells[shell_index].clone();
            return self.create_tab(shell);
        }

        self.dismiss_shell_picker();
        Task::none()
    }

    pub(super) fn handle_app_shortcut(
        &mut self,
        key: &Key,
        modifiers: Modifiers,
    ) -> Option<Task<Message>> {
        let action = ShortcutAction::resolve(key, modifiers, &self.config.shortcuts)?;

        match action {
            ShortcutAction::NewTab => Some(self.update(Message::OpenShellPicker)),
            ShortcutAction::CloseTab => {
                self.close_active_target();
                Some(Task::none())
            }
            ShortcutAction::OpenSettings => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
                Some(Task::none())
            }
            ShortcutAction::NextTab => {
                self.select_relative_tab(1);
                Some(Task::none())
            }
            ShortcutAction::PrevTab => {
                self.select_relative_tab(-1);
                Some(Task::none())
            }
            ShortcutAction::Quit => Some(iced::exit()),
            ShortcutAction::FontSizeIncrease => Some(self.adjust_font_size(1.0)),
            ShortcutAction::FontSizeDecrease => Some(self.adjust_font_size(-1.0)),
            ShortcutAction::FontSizeReset => {
                Some(self.set_font_size(crate::config::DEFAULT_TERMINAL_FONT_SIZE))
            }
            ShortcutAction::DuplicateTab => Some(self.update(Message::DuplicateTab)),
        }
    }

    fn adjust_font_size(&mut self, delta: f32) -> Task<Message> {
        let new_size = self.config.terminal.font_size + delta;
        self.set_font_size(new_size)
    }

    fn set_font_size(&mut self, size: f32) -> Task<Message> {
        let updates = crate::config::AppConfigUpdates {
            terminal_font_size: Some(size),
            ..Default::default()
        };
        let task = self.apply_updates_to_runtime(updates);
        self.queue_config_save();
        task
    }

    fn close_active_target(&mut self) {
        if self.active_tab == SETTINGS_TAB_INDEX {
            self.settings_open = false;
            self.active_tab = self.tabs.len().saturating_sub(1);
            return;
        }

        if self.tabs.is_empty() {
            return;
        }

        let index = self.active_tab.min(self.tabs.len() - 1);
        self.tabs.remove(index);
        self.clamp_active_tab();
    }

    fn select_relative_tab(&mut self, step: isize) {
        let mut visible_tabs: Vec<usize> = (0..self.tabs.len()).collect();
        if self.settings_open {
            visible_tabs.push(SETTINGS_TAB_INDEX);
        }

        if visible_tabs.is_empty() {
            return;
        }

        let current_pos = visible_tabs
            .iter()
            .position(|index| *index == self.active_tab)
            .unwrap_or(0);

        let len = visible_tabs.len() as isize;
        let next_pos = (current_pos as isize + step).rem_euclid(len) as usize;
        self.active_tab = visible_tabs[next_pos];
    }
}
