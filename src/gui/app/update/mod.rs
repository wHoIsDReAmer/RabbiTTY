mod settings;
mod tab;
mod terminal;

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::settings::SettingsDraft;
use crate::gui::tab::ShellKind;
use iced::keyboard::{Key, key::Named};
use iced::{Task, widget, window};
use std::sync::LazyLock;

pub(in crate::gui) static TAB_BAR_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);
pub(in crate::gui) static TERMINAL_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // ── Tab management ──────────────────────────────────────
            Message::TabSelected(index) => {
                if index == SETTINGS_TAB_INDEX && self.settings_open {
                    self.active_tab = SETTINGS_TAB_INDEX;
                } else if index < self.tabs.len() {
                    self.active_tab = index;
                }
            }
            Message::CloseTab(index) => {
                self.handle_close_tab(index);
            }
            Message::OpenShellPicker => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
            }
            Message::CloseShellPicker => {
                self.show_shell_picker = false;
                self.shell_picker_selected = 0;
            }
            Message::CreateTab(shell) => {
                return self.create_tab(shell);
            }
            Message::CreateSshTab(profile_index) => {
                if let Some(profile) = self.config.ssh_profiles.get(profile_index) {
                    let shell = ShellKind::Ssh(profile.clone());
                    return self.create_tab(shell);
                }
            }

            // ── Settings ────────────────────────────────────────────
            Message::AddSshProfile => {
                self.config
                    .ssh_profiles
                    .push(crate::config::SshProfile::default());
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            Message::RemoveSshProfile(index) => {
                if index < self.config.ssh_profiles.len() {
                    self.config.ssh_profiles.remove(index);
                    self.settings_draft = SettingsDraft::from_config(&self.config);
                }
            }
            Message::SshProfileFieldChanged(index, field, value) => {
                self.settings_draft.update_ssh_profile(index, field, value);
            }
            Message::SaveSshProfiles => {
                self.settings_draft
                    .apply_ssh_profiles_to(&mut self.config.ssh_profiles);
                if let Err(err) = self.config.save() {
                    eprintln!("Failed to save config: {err}");
                }
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            Message::OpenSettingsTab => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            Message::SelectSettingsCategory(category) => {
                self.settings_category = category;
                if !self.settings_open {
                    self.settings_open = true;
                    self.active_tab = SETTINGS_TAB_INDEX;
                    self.settings_draft = SettingsDraft::from_config(&self.config);
                }
            }
            Message::SettingsInputChanged(field, value) => {
                self.settings_draft.update(field, value);
            }
            Message::SettingsBlurToggled(enabled) => {
                self.settings_draft.blur_enabled = enabled;
            }
            Message::ApplySettings => {
                return self.apply_settings(false);
            }
            Message::SaveSettings => {
                return self.apply_settings(true);
            }
            #[cfg(target_os = "macos")]
            Message::ConfirmRestartForBlur => {
                return self.handle_confirm_restart();
            }
            #[cfg(target_os = "macos")]
            Message::CancelRestartForBlur => {
                self.show_restart_confirm = false;
                self.pending_settings_updates = None;
                self.pending_save_on_restart = false;
            }

            // ── Terminal / PTY ──────────────────────────────────────
            Message::PtySenderReady(sender) => {
                self.pty_sender = Some(sender);
            }
            Message::PtyOutput(event) => {
                self.handle_pty_event(event);
                self.ignore_scrollable_sync = true;
                return self.sync_terminal_scrollable();
            }
            Message::PtyOutputBatch(events) => {
                for event in events {
                    self.handle_pty_event(event);
                }
                self.ignore_scrollable_sync = true;
                return self.sync_terminal_scrollable();
            }
            Message::KeyPressed {
                key,
                modifiers,
                text,
            } => {
                return self.handle_key_pressed(key, modifiers, text);
            }
            Message::TabBarScroll(delta) => {
                return self.handle_tab_bar_scroll(delta);
            }
            Message::TerminalScroll(rel_y) => {
                if self.ignore_scrollable_sync {
                    self.ignore_scrollable_sync = false;
                } else if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    tab.scroll_to_relative(rel_y);
                }
            }
            Message::TerminalWheelScroll(delta) => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    tab.scroll(delta);
                }
                self.ignore_scrollable_sync = true;
                return self.sync_terminal_scrollable_forced();
            }
            Message::WindowResized(size) => {
                self.handle_window_resized(size);
            }

            // ── Window ──────────────────────────────────────────────
            Message::Exit => {
                return iced::exit();
            }
            Message::ApplyWindowStyle => {
                return self.handle_apply_window_style();
            }
            #[cfg(target_os = "windows")]
            Message::WindowMinimize => {
                return window::latest().and_then(|id| window::minimize(id, true));
            }
            #[cfg(target_os = "windows")]
            Message::WindowMaximize => {
                return window::latest().and_then(window::toggle_maximize);
            }
            #[cfg(target_os = "windows")]
            Message::WindowDrag => {
                return window::latest().and_then(window::drag);
            }
        }

        Task::none()
    }

    fn handle_key_pressed(
        &mut self,
        key: Key,
        modifiers: iced::keyboard::Modifiers,
        text: Option<String>,
    ) -> Task<Message> {
        if self.show_shell_picker {
            match key {
                Key::Named(Named::Escape) => {
                    self.show_shell_picker = false;
                    self.shell_picker_selected = 0;
                }
                Key::Named(Named::ArrowUp) => {
                    self.shift_shell_picker_selection(-1);
                }
                Key::Named(Named::ArrowDown) => {
                    self.shift_shell_picker_selection(1);
                }
                Key::Named(Named::Enter) => {
                    return self.confirm_shell_picker_selection();
                }
                _ => {}
            }
            return Task::none();
        }

        if let Some(task) = self.handle_app_shortcut(&key, modifiers) {
            return task;
        }

        if self.active_tab == SETTINGS_TAB_INDEX {
            return Task::none();
        }
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.handle_key(&key, modifiers, text.as_deref());
        }
        self.ignore_scrollable_sync = true;
        self.sync_terminal_scrollable()
    }

    fn handle_apply_window_style(&mut self) -> Task<Message> {
        if self.window_style_applied {
            return Task::none();
        }
        self.window_style_applied = true;

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let theme = self.config.theme.clone();
            window::latest()
                .and_then(move |id| {
                    let theme = theme.clone();
                    window::run(id, move |window| {
                        if let Ok(handle) = window.window_handle() {
                            crate::platform::apply_style(handle, &theme);
                        }
                    })
                })
                .discard()
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            Task::none()
        }
    }
}
