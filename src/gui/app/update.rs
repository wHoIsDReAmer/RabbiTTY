use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::config::AppConfigUpdates;
use crate::gui::settings::SettingsDraft;
use crate::gui::tab::ShellKind;
use crate::session::OutputEvent;
use crate::terminal::TerminalTheme;
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::sink::SinkExt;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::stream;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Event, Size, Subscription, Task, event, mouse, widget, window};

use std::sync::LazyLock;

use super::shortcuts::ShortcutAction;

pub(in crate::gui) static TAB_BAR_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) => {
                if index == SETTINGS_TAB_INDEX && self.settings_open {
                    self.active_tab = SETTINGS_TAB_INDEX;
                } else if index < self.tabs.len() {
                    self.active_tab = index;
                }
            }
            Message::CloseTab(index) => {
                if index == SETTINGS_TAB_INDEX {
                    self.settings_open = false;
                    if self.active_tab == SETTINGS_TAB_INDEX {
                        self.active_tab = self.tabs.len().saturating_sub(1);
                    }
                } else if index < self.tabs.len() {
                    self.tabs.remove(index);

                    if self.active_tab != SETTINGS_TAB_INDEX {
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                        if self.tabs.is_empty() {
                            self.active_tab = 0;
                        }
                    }
                }
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

                return Task::none();
            }
            #[cfg(target_os = "macos")]
            Message::CancelRestartForBlur => {
                self.show_restart_confirm = false;
                self.pending_settings_updates = None;
                self.pending_save_on_restart = false;
                return Task::none();
            }
            Message::PtySenderReady(sender) => {
                self.pty_sender = Some(sender);
            }
            Message::PtyOutput(event) => {
                self.handle_pty_event(event);
            }
            Message::PtyOutputBatch(events) => {
                for event in events {
                    self.handle_pty_event(event);
                }
            }
            Message::KeyPressed {
                key,
                modifiers,
                text,
            } => {
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
            }
            Message::Exit => {
                return iced::exit();
            }
            Message::ApplyWindowStyle => {
                if self.window_style_applied {
                    return Task::none();
                }
                self.window_style_applied = true;

                #[cfg(any(target_os = "windows", target_os = "macos"))]
                {
                    let theme = self.config.theme.clone();
                    return window::latest()
                        .and_then(move |id| {
                            let theme = theme.clone();
                            window::run(id, move |window| {
                                if let Ok(handle) = window.window_handle() {
                                    crate::platform::apply_style(handle, &theme);
                                }
                            })
                        })
                        .discard();
                }

                #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                {
                    return Task::none();
                }
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

            Message::TabBarScroll(delta) => {
                let tab_count = self.tabs.len() + if self.settings_open { 1 } else { 0 };
                let max_offset = (tab_count as f32 * 150.0).max(0.0);

                self.tab_bar_scroll_offset =
                    (self.tab_bar_scroll_offset + delta).clamp(0.0, max_offset);

                return scroll_to(
                    TAB_BAR_SCROLLABLE_ID.clone(),
                    scrollable::AbsoluteOffset {
                        x: self.tab_bar_scroll_offset,
                        y: 0.0,
                    },
                );
            }
            Message::WindowResized(size) => {
                self.window_size = size;

                let previous_width = self.config.ui.window_width;
                let previous_height = self.config.ui.window_height;
                let updates = AppConfigUpdates {
                    window_width: Some(size.width),
                    window_height: Some(size.height),
                    ..Default::default()
                };
                self.config.apply_updates(updates);
                self.settings_draft
                    .sync_window_size(self.config.ui.window_width, self.config.ui.window_height);
                if ((self.config.ui.window_width - previous_width).abs() > f32::EPSILON
                    || (self.config.ui.window_height - previous_height).abs() > f32::EPSILON)
                    && let Err(err) = self.config.save()
                {
                    eprintln!("Failed to save config: {err}");
                }

                let (cols, rows) = self.grid_for_size(size);

                for tab in &mut self.tabs {
                    tab.resize(cols, rows);
                }
            }
        }

        Task::none()
    }

    fn handle_pty_event(&mut self, event: OutputEvent) {
        match event {
            OutputEvent::Data { tab_id, bytes } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.feed_bytes(&bytes);
                }
            }
            OutputEvent::Closed { tab_id } => {
                if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
                    self.tabs.remove(index);
                    if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active_tab = self.tabs.len() - 1;
                    }
                }
            }
        }
    }

    fn apply_settings(&mut self, save: bool) -> Task<Message> {
        let updates = self.settings_draft.to_updates();

        #[cfg(target_os = "macos")]
        if let Some(new_enabled) = updates.blur_enabled
            && new_enabled != self.config.theme.blur_enabled
        {
            self.show_restart_confirm = true;
            self.pending_settings_updates = Some(updates);
            self.pending_save_on_restart = true;
            return Task::none();
        }

        let resize_task = self.apply_updates_to_runtime(updates);

        if save && let Err(err) = self.config.save() {
            eprintln!("Failed to save config: {err}");
        }

        resize_task
    }

    fn apply_updates_to_runtime(&mut self, updates: AppConfigUpdates) -> Task<Message> {
        self.config.apply_updates(updates);
        self.settings_draft = SettingsDraft::from_config(&self.config);

        let new_size = Size::new(self.config.ui.window_width, self.config.ui.window_height);
        let resize_task = if (self.window_size.width - new_size.width).abs() > f32::EPSILON
            || (self.window_size.height - new_size.height).abs() > f32::EPSILON
        {
            self.window_size = new_size;
            window::latest().and_then(move |id| window::resize(id, new_size))
        } else {
            Task::none()
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        for tab in &mut self.tabs {
            tab.resize(cols, rows);
            tab.set_theme(theme.clone());
        }

        resize_task
    }

    fn create_tab(&mut self, shell: ShellKind) -> Task<Message> {
        let Some(sender) = self.pty_sender.clone() else {
            eprintln!("PTY output channel not ready");
            return Task::none();
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let new_tab =
            crate::gui::tab::TerminalTab::from_shell(shell, cols, rows, theme, tab_id, sender);
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        Task::none()
    }

    pub(super) fn shell_picker_option_count(&self) -> usize {
        let ssh_count = self.config.ssh_profiles.len();

        #[cfg(target_family = "unix")]
        {
            1 + ssh_count + 1 // shell + ssh profiles + cancel
        }

        #[cfg(target_family = "windows")]
        {
            2 + ssh_count + 1 // cmd + powershell + ssh profiles + cancel
        }
    }

    fn shift_shell_picker_selection(&mut self, delta: isize) {
        let count = self.shell_picker_option_count() as isize;
        if count <= 0 {
            return;
        }

        let next = (self.shell_picker_selected as isize + delta).rem_euclid(count) as usize;
        self.shell_picker_selected = next;
    }

    fn confirm_shell_picker_selection(&mut self) -> Task<Message> {
        let selected = self.shell_picker_selected;

        #[cfg(target_family = "unix")]
        let shell_count = 1usize;
        #[cfg(target_family = "windows")]
        let shell_count = 2usize;

        // Shell options first
        if selected < shell_count {
            #[cfg(target_family = "unix")]
            {
                return self.create_tab(ShellKind::Zsh);
            }

            #[cfg(target_family = "windows")]
            {
                return match selected {
                    0 => self.create_tab(ShellKind::Cmd),
                    _ => self.create_tab(ShellKind::PowerShell),
                };
            }
        }

        // SSH profiles
        let ssh_index = selected - shell_count;
        if ssh_index < self.config.ssh_profiles.len() {
            let profile = self.config.ssh_profiles[ssh_index].clone();
            return self.create_tab(ShellKind::Ssh(profile));
        }

        // Cancel
        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        Task::none()
    }

    fn handle_app_shortcut(&mut self, key: &Key, modifiers: Modifiers) -> Option<Task<Message>> {
        let action = ShortcutAction::resolve(key, modifiers, &self.config.shortcuts)?;

        match action {
            ShortcutAction::NewTab => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
                Some(Task::none())
            }
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
        }
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

        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
        if self.tabs.is_empty() {
            self.active_tab = 0;
        }
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

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(|| {
                stream::channel(100, async |mut output| {
                    let (sender, mut receiver) = mpsc::channel(100);
                    let _ = output.send(Message::PtySenderReady(sender)).await;

                    while let Some(first) = receiver.next().await {
                        let mut batch = vec![first];
                        // Drain all pending events without blocking
                        while let Ok(event) = receiver.try_recv() {
                            batch.push(event);
                        }
                        if batch.len() == 1 {
                            if output
                                .send(Message::PtyOutput(batch.pop().unwrap()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        } else if output.send(Message::PtyOutputBatch(batch)).await.is_err() {
                            break;
                        }
                    }
                })
            }),
            event::listen_with(|event, _status, _id| match event {
                Event::Window(window::Event::CloseRequested) => Some(Message::Exit),
                Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                }) => Some(Message::KeyPressed {
                    key,
                    modifiers,
                    text: text.map(|s| s.to_string()),
                }),
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    let pixels = match delta {
                        mouse::ScrollDelta::Lines { x, y } => x * 30.0 - y * 30.0,
                        mouse::ScrollDelta::Pixels { x, y } => x - y,
                    };
                    if pixels.abs() > 0.1 {
                        Some(Message::TabBarScroll(pixels))
                    } else {
                        None
                    }
                }
                _ => None,
            }),
        ])
    }
}
