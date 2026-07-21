use super::super::{App, Message, SETTINGS_TAB_INDEX, SettingsMessage};
use crate::config::AppConfigUpdates;
use crate::gui::settings::SettingsDraft;
use crate::gui::settings::SettingsField;
use crate::terminal::TerminalTheme;
use iced::time::Instant;
use iced::{Size, Task, window};

impl App {
    pub(super) fn apply_settings(&mut self, save: bool) -> Task<Message> {
        let updates = self.settings_draft.to_updates();

        #[cfg(target_os = "macos")]
        {
            let blur_toggled = updates
                .blur_enabled
                .is_some_and(|v| v != self.config.theme.blur_enabled);
            let radius_changed = updates
                .macos_blur_radius
                .is_some_and(|v| v != self.config.theme.macos_blur_radius);
            if blur_toggled || radius_changed {
                self.show_restart_confirm = true;
                self.pending_settings_updates = Some(updates);
                self.pending_save_on_restart = save;
                return Task::none();
            }
        }

        let resize_task = self.apply_updates_to_runtime(updates);

        if save {
            self.queue_config_save();
        }

        resize_task
    }

    pub(super) fn queue_config_save(&self) {
        let _ = self.config_save_tx.send(self.config.clone());
    }

    pub(super) fn apply_updates_to_runtime(&mut self, updates: AppConfigUpdates) -> Task<Message> {
        let affects_locale = updates.language.is_some();
        let affects_theme = updates.color_scheme.is_some()
            || updates.foreground.is_some()
            || updates.background.is_some()
            || updates.cursor.is_some()
            || updates.ansi_colors.is_some()
            || updates.terminal_bold_is_bright.is_some()
            || updates.background_opacity.is_some()
            || updates.blur_enabled.is_some()
            || updates.macos_blur_radius.is_some();
        let affects_grid = updates.window_width.is_some()
            || updates.window_height.is_some()
            || updates.terminal_font_selection.is_some()
            || updates.terminal_font_size.is_some()
            || updates.terminal_padding_x.is_some()
            || updates.terminal_padding_y.is_some();
        let affects_window = updates.window_width.is_some() || updates.window_height.is_some();

        self.config.apply_updates(updates);
        self.settings_draft = SettingsDraft::from_config(&self.config);

        if affects_locale {
            crate::i18n::set_locale(self.config.ui.language.as_deref());
        }
        if affects_theme {
            self.palette = crate::gui::theme::Palette::from_theme(&self.config.theme);
        }

        let resize_task = if affects_window {
            let new_size = Size::new(self.config.ui.window_width, self.config.ui.window_height);
            if (self.window_size.width - new_size.width).abs() > f32::EPSILON
                || (self.window_size.height - new_size.height).abs() > f32::EPSILON
            {
                self.window_size = new_size;
                window::latest().and_then(move |id| window::resize(id, new_size))
            } else {
                Task::none()
            }
        } else {
            Task::none()
        };

        if affects_grid || affects_theme {
            let (cols, rows) = self.grid_for_size(self.window_size);
            let theme = affects_theme.then(|| TerminalTheme::from_config(&self.config));
            for tab in &mut self.tabs {
                if affects_grid {
                    let current = tab.size();
                    if current.columns != cols || current.lines != rows {
                        tab.resize(cols, rows);
                    }
                }
                if let Some(ref theme) = theme {
                    tab.set_theme(theme.clone());
                }
            }
        }

        resize_task
    }

    #[cfg(target_os = "macos")]
    pub(super) fn handle_confirm_restart(&mut self) -> Task<Message> {
        if let Some(updates) = self.pending_settings_updates.take() {
            let _ = self.apply_updates_to_runtime(updates);
            if self.pending_save_on_restart
                && let Err(err) = self.config.save()
            {
                eprintln!("Failed to save config: {err}");
            }
        }

        let restart_spawned = match std::env::current_exe() {
            Ok(current_exe) => {
                let args: Vec<_> = std::env::args_os().skip(1).collect();
                match std::process::Command::new(current_exe).args(args).spawn() {
                    Ok(_) => true,
                    Err(err) => {
                        eprintln!("Failed to relaunch app: {err}");
                        false
                    }
                }
            }
            Err(err) => {
                eprintln!("Failed to locate executable for restart: {err}");
                false
            }
        };

        self.show_restart_confirm = false;
        self.pending_save_on_restart = false;

        if restart_spawned {
            return iced::exit();
        }

        Task::none()
    }
}

impl App {
    pub(super) fn update_settings_message(&mut self, message: SettingsMessage) -> Task<Message> {
        match message {
            SettingsMessage::AddProfile => {
                self.settings_draft.open_create_profile_modal();
            }
            SettingsMessage::EditProfile(index) => {
                self.settings_draft.open_edit_profile_modal(index);
            }
            SettingsMessage::LaunchProfile(index) => {
                if let Some(profile) = self
                    .settings_draft
                    .profiles
                    .get(index)
                    .and_then(|draft| draft.to_profile())
                {
                    return self.launch_profile(profile);
                }
            }
            SettingsMessage::RequestRemoveProfile(index) => {
                self.settings_draft.request_delete_profile(index);
            }
            SettingsMessage::CancelRemoveProfile => {
                self.settings_draft.cancel_delete_profile();
            }
            SettingsMessage::ConfirmRemoveProfile => {
                if let Some(removed) = self.settings_draft.confirm_delete_profile() {
                    if let Some(ssh) = removed.to_ssh_profile() {
                        crate::keychain::delete_password(&ssh.host, &ssh.user);
                    }
                    self.save_profiles();
                }
            }
            SettingsMessage::ProfileModalFieldChanged(field, value) => {
                self.settings_draft.update_profile_modal(field, value);
            }
            SettingsMessage::ProfileModalTypeSelected(kind) => {
                self.settings_draft.set_profile_modal_kind(kind);
            }
            SettingsMessage::ProfileModalTabSelected(tab) => {
                self.settings_draft.set_profile_modal_tab(tab);
            }
            SettingsMessage::ProfileModalBaseSelected(index) => {
                let base = index.and_then(|i| self.ssh_config_profiles.get(i));
                self.settings_draft.apply_profile_modal_base(index, base);
            }
            SettingsMessage::TestSshConnection => match self
                .settings_draft
                .begin_ssh_connection_test()
            {
                Ok(profile) => {
                    return Task::perform(
                        crate::ssh::test_ssh_connection(profile, std::time::Duration::from_secs(5)),
                        |r| Message::Settings(SettingsMessage::SshConnectionTestFinished(r)),
                    );
                }
                Err(err) => {
                    eprintln!("Failed to start SSH connection test: {err}");
                }
            },
            SettingsMessage::SshConnectionTestFinished(result) => {
                self.settings_draft.finish_ssh_connection_test(result);
            }
            SettingsMessage::CloseProfileModal => {
                self.settings_draft.close_profile_modal();
            }
            SettingsMessage::SaveProfileModal => match self.settings_draft.save_profile_modal() {
                Ok(Some(profile)) => {
                    if let Some(ssh) = profile.ssh_profile() {
                        match ssh.password.as_deref() {
                            Some(pw) => {
                                crate::keychain::set_password(&ssh.host, &ssh.user, pw);
                            }
                            None => {
                                crate::keychain::delete_password(&ssh.host, &ssh.user);
                            }
                        }
                    }
                    self.save_profiles();
                }
                Ok(None) => {}
                Err(err) => eprintln!("Failed to update profile draft: {err}"),
            },
            SettingsMessage::OpenTab => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            SettingsMessage::SelectCategory(category) => {
                if !self.settings_open {
                    self.settings_open = true;
                    self.active_tab = SETTINGS_TAB_INDEX;
                    self.settings_draft = SettingsDraft::from_config(&self.config);
                }
                if let Some(immediate) = self.settings_category_transition.request_switch(
                    category,
                    self.settings_category,
                    self.config.ui.animations_enabled,
                    Instant::now(),
                ) {
                    self.settings_category = immediate;
                }
            }
            SettingsMessage::InputChanged(field, value) => {
                self.settings_draft.update(field, value);
                self.settings_debounce_seq = self.settings_debounce_seq.wrapping_add(1);
                if self.settings_debounce_pending {
                    return Task::none();
                }
                self.settings_debounce_pending = true;
                self.settings_debounce_spawned_seq = self.settings_debounce_seq;
                return Task::perform(
                    async {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    },
                    |()| Message::Settings(SettingsMessage::CommitDebounce),
                );
            }
            SettingsMessage::InputCommitted(field, value) => {
                self.settings_draft.update(field, value);
                self.settings_debounce_spawned_seq = self.settings_debounce_seq;
                return self.apply_settings(true);
            }
            SettingsMessage::CommitDebounce => {
                if self.settings_debounce_spawned_seq != self.settings_debounce_seq {
                    self.settings_debounce_spawned_seq = self.settings_debounce_seq;
                    return Task::perform(
                        async {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        },
                        |()| Message::Settings(SettingsMessage::CommitDebounce),
                    );
                }
                self.settings_debounce_pending = false;
                return self.apply_settings(true);
            }
            SettingsMessage::BlurToggled(enabled) => {
                self.settings_draft.blur_enabled = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::AnimationsToggled(enabled) => {
                self.settings_draft.animations_enabled = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::TabBarPositionSelected(pos) => {
                self.settings_draft.tab_bar_position = pos;
                return self.apply_settings(true);
            }
            SettingsMessage::BracketedPasteToggled(enabled) => {
                self.settings_draft.bracketed_paste = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::MultilinePasteConfirmToggled(enabled) => {
                self.settings_draft.multiline_paste_confirm = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::CursorShapeSelected(shape) => {
                self.settings_draft.cursor_shape = shape;
                return self.apply_settings(true);
            }
            SettingsMessage::CursorBlinkToggled(enabled) => {
                self.settings_draft.cursor_blink = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::BoldIsBrightToggled(enabled) => {
                self.settings_draft.bold_is_bright = enabled;
                return self.apply_settings(true);
            }
            SettingsMessage::BellModeSelected(mode) => {
                self.settings_draft.bell_mode = mode;
                return self.apply_settings(true);
            }
            SettingsMessage::RightClickActionSelected(action) => {
                self.settings_draft.right_click_action = action;
                return self.apply_settings(true);
            }
            SettingsMessage::FontSelected(option) => {
                self.settings_draft
                    .update(SettingsField::TerminalFontSelection, option.value);
                return self.apply_settings(true);
            }
            SettingsMessage::ToggleShowAllFonts(show_all) => {
                self.show_all_fonts = show_all;
                self.font_combo_state = super::super::build_font_combo_state(
                    &self.all_font_options,
                    show_all,
                    self.config.terminal.font_selection.as_deref(),
                );
                return self.apply_settings(true);
            }
            #[cfg(target_os = "macos")]
            SettingsMessage::ConfirmRestartForBlur => {
                return self.handle_confirm_restart();
            }
            #[cfg(target_os = "macos")]
            SettingsMessage::CancelRestartForBlur => {
                self.show_restart_confirm = false;
                self.pending_settings_updates = None;
                self.pending_save_on_restart = false;
            }
        }
        Task::none()
    }
}
