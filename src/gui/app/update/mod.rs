mod settings;
mod sftp;
mod tab;
mod terminal;

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::settings::{SettingsDraft, SettingsField};
use crate::gui::tab::ShellKind;
use iced::keyboard::{Key, key::Named};
use iced::time::Instant;
use iced::{Task, widget};
use std::sync::LazyLock;

pub(in crate::gui) static TAB_BAR_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);
pub(in crate::gui) static TERMINAL_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);

const IGNORE_SCROLL_SYNC_COUNT: u8 = 2;

impl App {
    fn active_session_mut(&mut self) -> Option<&mut crate::gui::tab::TerminalTab> {
        if self.active_tab == SETTINGS_TAB_INDEX {
            return None;
        }
        self.tabs
            .get_mut(self.active_tab)
            .filter(|tab| matches!(tab.session, crate::gui::tab::TerminalSession::Active(_)))
    }

    fn save_ssh_profiles(&mut self) {
        if let Err(err) = self
            .settings_draft
            .apply_ssh_profiles_to(&mut self.config.ssh_profiles)
        {
            eprintln!("Failed to save SSH profiles: {err}");
            return;
        }

        match self.config.save() {
            Ok(()) => {
                self.settings_draft = SettingsDraft::from_config(&self.config);
                self.settings_draft.set_ssh_profiles_saved();
            }
            Err(err) => {
                let message = format!("Failed to save SSH profiles: {err}");
                eprintln!("{message}");
                self.settings_draft.set_ssh_profiles_error(message);
            }
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Noop => {}

            // ── Tab management ──────────────────────────────────────
            Message::TabSelected(index) => {
                if index == SETTINGS_TAB_INDEX && self.settings_open {
                    self.active_tab = SETTINGS_TAB_INDEX;
                } else if index < self.tabs.len() {
                    self.active_tab = index;
                    self.dragging_tab = Some(index);
                    self.drag_target = None;
                }
            }
            Message::TabDragHover(index) => {
                if self.dragging_tab.is_some() && index < self.tabs.len() {
                    self.drag_target = Some(index);
                }
            }
            Message::TabDragRelease => {
                if let Some(from) = self.dragging_tab.take()
                    && let Some(target) = self.drag_target.take()
                    && from != target
                    && from < self.tabs.len()
                    && target < self.tabs.len()
                {
                    let tab = self.tabs.remove(from);
                    self.tabs.insert(target, tab);
                    if self.active_tab == from {
                        self.active_tab = target;
                    } else if from < self.active_tab && target >= self.active_tab {
                        self.active_tab -= 1;
                    } else if from > self.active_tab && target <= self.active_tab {
                        self.active_tab += 1;
                    }
                }
                self.drag_target = None;
            }
            Message::CloseTab(index) => {
                self.tab_context_menu = None;
                self.handle_close_tab(index);
            }
            Message::OpenShellPicker => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
                self.shell_picker_anim.go_mut(true, Instant::now());
            }
            Message::CloseShellPicker => {
                self.shell_picker_anim.go_mut(false, Instant::now());
            }
            Message::CreateTab(shell) => match shell {
                ShellKind::Ssh(profile) => return self.request_ssh_tab(profile),
                shell => return self.create_tab(shell),
            },
            Message::CreateSshTab(profile_index) => {
                if let Some(profile) = self.session_ssh_profiles().get(profile_index).cloned() {
                    return self.request_ssh_tab(profile);
                }
            }
            Message::LaunchFromHistory(index) => {
                if let Some(entry) = self.session_history.entries.get(index).cloned()
                    && let Some(shell) = entry.kind.to_shell_kind(&self.config.ssh_profiles)
                {
                    return match shell {
                        ShellKind::Ssh(profile) => self.request_ssh_tab(profile),
                        shell => self.create_tab(shell),
                    };
                }
            }
            Message::DuplicateTab => {
                let index = self.tab_context_menu.unwrap_or(self.active_tab);
                self.tab_context_menu = None;
                if let Some(tab) = self.tabs.get(index) {
                    let shell = tab.shell.clone();
                    return self.create_tab(shell);
                }
            }
            Message::SftpToggleDrawer => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && matches!(tab.shell, ShellKind::Ssh(_))
                {
                    let was_open = tab.sftp.open;
                    tab.sftp.anim.go_mut(!was_open, Instant::now());
                    if !was_open {
                        tab.sftp.open = true;
                    }
                    if !was_open
                        && tab.sftp.command_tx.is_none()
                        && !tab.sftp.opening
                        && let crate::gui::tab::TerminalSession::Active(session) = &tab.session
                        && let Some(ssh) = session.ssh_handle()
                    {
                        tab.sftp.opening = true;
                        tab.sftp.error = None;
                        let tab_id = tab.id;
                        return sftp::open_sftp_stream(ssh.clone(), tab_id);
                    }
                }
            }
            Message::SftpOpenSucceeded { tab_id, command_tx } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp.opening = false;
                    tab.sftp.command_tx = Some(command_tx.clone());
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let path = tab.sftp.current_path.clone();
                    let _ = command_tx.unbounded_send(crate::ssh::sftp::Command::List(path));
                }
            }
            Message::SftpOpenFailed { tab_id, error } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp.opening = false;
                    tab.sftp.error = Some(error);
                }
            }
            Message::SftpEvent { tab_id, event } => {
                let finished_path =
                    if let crate::ssh::sftp::Event::TransferFinished { path } = &event {
                        Some(path.clone())
                    } else {
                        None
                    };
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    sftp::apply_sftp_event(&mut tab.sftp, event);
                }
                if let Some(path) = finished_path {
                    return Task::perform(
                        async {
                            std::thread::sleep(std::time::Duration::from_millis(1500));
                        },
                        move |()| Message::SftpDismissTransfer {
                            tab_id,
                            path: path.clone(),
                        },
                    );
                }
            }
            Message::SftpDismissTransfer { tab_id, path } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                    tab.sftp
                        .transfers
                        .retain(|t| !(t.finished && t.path == path));
                }
            }
            Message::SshPasswordPromptChanged(value) => {
                if let Some(prompt) = self.password_prompt.as_mut() {
                    prompt.draft = value;
                    prompt.error = None;
                }
            }
            Message::SshPasswordPromptToggleSave(save) => {
                if let Some(prompt) = self.password_prompt.as_mut() {
                    prompt.save_to_keychain = save;
                }
            }
            Message::SshPasswordPromptSubmit => {
                if let Some(prompt) = self.password_prompt.take() {
                    let mut profile = prompt.profile;
                    profile.password = Some(prompt.draft.clone());
                    if prompt.save_to_keychain {
                        crate::keychain::set_password(&profile.host, &profile.user, &prompt.draft);
                    }
                    return self.create_tab(crate::gui::tab::ShellKind::Ssh(profile));
                }
            }
            Message::SshPasswordPromptCancel => {
                self.password_prompt = None;
            }
            Message::CreateSshTabFromConfig(index) => {
                if let Some(profile) = self.ssh_config_profiles.get(index).cloned() {
                    return self.request_ssh_tab(profile);
                }
            }
            Message::SftpNavigate { tab_id, path } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let _ = tx.unbounded_send(crate::ssh::sftp::Command::List(path));
                }
            }
            Message::SftpRefresh => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    tab.sftp.loading = true;
                    tab.sftp.error = None;
                    let _ = tx.unbounded_send(crate::ssh::sftp::Command::List(
                        tab.sftp.current_path.clone(),
                    ));
                }
            }
            Message::SftpRequestUpload => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get(self.active_tab)
                    && matches!(tab.shell, ShellKind::Ssh(_))
                {
                    let tab_id = tab.id;
                    return Task::perform(
                        async move {
                            rfd::AsyncFileDialog::new()
                                .pick_files()
                                .await
                                .map(|files| {
                                    files
                                        .into_iter()
                                        .map(|f| f.path().to_path_buf())
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default()
                        },
                        move |files| Message::SftpUploadPicked { tab_id, files },
                    );
                }
            }
            Message::SftpUploadPicked { tab_id, files } => {
                if files.is_empty() {
                    return Task::none();
                }
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let base = tab.sftp.current_path.clone();
                    for local in files {
                        let name = local
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("upload")
                            .to_string();
                        let remote = crate::gui::sftp::join_path(&base, &name);
                        let _ =
                            tx.unbounded_send(crate::ssh::sftp::Command::Upload { local, remote });
                    }
                }
            }
            Message::SftpRequestDownload {
                tab_id,
                remote,
                suggested_name,
            } => {
                return Task::perform(
                    async move {
                        let local = rfd::AsyncFileDialog::new()
                            .set_file_name(&suggested_name)
                            .save_file()
                            .await
                            .map(|f| f.path().to_path_buf());
                        (remote, local)
                    },
                    move |(remote, local)| match local {
                        Some(local) => Message::SftpDownloadPicked {
                            tab_id,
                            remote,
                            local,
                        },
                        None => Message::Noop,
                    },
                );
            }
            Message::SftpDownloadPicked {
                tab_id,
                remote,
                local,
            } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let _ =
                        tx.unbounded_send(crate::ssh::sftp::Command::Download { remote, local });
                }
            }
            Message::SftpCancelTransfer => {
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(tx) = tab.sftp.command_tx.clone()
                {
                    let _ = tx.unbounded_send(crate::ssh::sftp::Command::Cancel);
                }
            }
            Message::ShowTabContextMenu(index) => {
                self.tab_context_menu = Some(index);
            }
            Message::CloseTabContextMenu => {
                self.tab_context_menu = None;
            }
            Message::TerminalRightClick => {
                use crate::config::RightClickAction;
                match self.config.terminal.right_click_action {
                    RightClickAction::Paste => {
                        return iced::clipboard::read()
                            .map(|content| Message::PasteClipboard(content.unwrap_or_default()));
                    }
                    RightClickAction::Menu => {
                        self.terminal_context_menu = true;
                    }
                    RightClickAction::None => {}
                }
            }
            Message::CloseTerminalContextMenu => {
                self.terminal_context_menu = false;
            }
            Message::TerminalContextPaste => {
                self.terminal_context_menu = false;
                return iced::clipboard::read()
                    .map(|content| Message::PasteClipboard(content.unwrap_or_default()));
            }
            Message::TerminalContextCopy => {
                self.terminal_context_menu = false;
                if let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(text) = tab.selected_text()
                {
                    tab.clear_selection();
                    return iced::clipboard::write(text);
                }
            }
            Message::CursorMoved(point) => {
                if self.tab_context_menu.is_none() && !self.terminal_context_menu {
                    self.cursor_position = point;
                }
            }

            // ── Settings ────────────────────────────────────────────
            Message::AddSshProfile => {
                self.settings_draft.open_create_ssh_profile_modal();
            }
            Message::EditSshProfile(index) => {
                self.settings_draft.open_edit_ssh_profile_modal(index);
            }
            Message::RequestRemoveSshProfile(index) => {
                self.settings_draft.request_delete_ssh_profile(index);
            }
            Message::CancelRemoveSshProfile => {
                self.settings_draft.cancel_delete_ssh_profile();
            }
            Message::ConfirmRemoveSshProfile => {
                if let Some((host, user)) = self.settings_draft.confirm_delete_ssh_profile() {
                    crate::keychain::delete_password(&host, &user);
                    self.save_ssh_profiles();
                }
            }
            Message::SshProfileModalFieldChanged(field, value) => {
                self.settings_draft.update_ssh_profile_modal(field, value);
            }
            Message::TestSshConnection => match self.settings_draft.begin_ssh_connection_test() {
                Ok(profile) => {
                    return Task::perform(
                        crate::ssh::test_ssh_connection(profile, std::time::Duration::from_secs(5)),
                        Message::SshConnectionTestFinished,
                    );
                }
                Err(err) => {
                    eprintln!("Failed to start SSH connection test: {err}");
                }
            },
            Message::SshConnectionTestFinished(result) => {
                self.settings_draft.finish_ssh_connection_test(result);
            }
            Message::CloseSshProfileModal => {
                self.settings_draft.close_ssh_profile_modal();
            }
            Message::SaveSshProfileModal => match self.settings_draft.save_ssh_profile_modal() {
                Ok(Some(profile)) => {
                    match profile.password.as_deref() {
                        Some(pw) => {
                            crate::keychain::set_password(&profile.host, &profile.user, pw);
                        }
                        None => {
                            crate::keychain::delete_password(&profile.host, &profile.user);
                        }
                    }
                    self.save_ssh_profiles();
                }
                Ok(None) => {}
                Err(err) => eprintln!("Failed to update SSH profile draft: {err}"),
            },
            Message::OpenSettingsTab => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            Message::SelectSettingsCategory(category) => {
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
            Message::SettingsInputChanged(field, value) => {
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
                    |()| Message::SettingsCommitDebounce,
                );
            }
            Message::SettingsInputCommitted(field, value) => {
                self.settings_draft.update(field, value);
                self.settings_debounce_spawned_seq = self.settings_debounce_seq;
                return self.apply_settings(true);
            }
            Message::SettingsCommitDebounce => {
                if self.settings_debounce_spawned_seq != self.settings_debounce_seq {
                    self.settings_debounce_spawned_seq = self.settings_debounce_seq;
                    return Task::perform(
                        async {
                            std::thread::sleep(std::time::Duration::from_millis(500));
                        },
                        |()| Message::SettingsCommitDebounce,
                    );
                }
                self.settings_debounce_pending = false;
                return self.apply_settings(true);
            }
            Message::SettingsBlurToggled(enabled) => {
                self.settings_draft.blur_enabled = enabled;
                return self.apply_settings(true);
            }
            Message::SettingsAnimationsToggled(enabled) => {
                self.settings_draft.animations_enabled = enabled;
                return self.apply_settings(true);
            }
            Message::SettingsTabBarPositionSelected(pos) => {
                self.settings_draft.tab_bar_position = pos;
                return self.apply_settings(true);
            }
            Message::SettingsBracketedPasteToggled(enabled) => {
                self.settings_draft.bracketed_paste = enabled;
                return self.apply_settings(true);
            }
            Message::SettingsMultilinePasteConfirmToggled(enabled) => {
                self.settings_draft.multiline_paste_confirm = enabled;
                return self.apply_settings(true);
            }
            Message::SettingsCursorShapeSelected(shape) => {
                self.settings_draft.cursor_shape = shape;
                return self.apply_settings(true);
            }
            Message::SettingsCursorBlinkToggled(enabled) => {
                self.settings_draft.cursor_blink = enabled;
                return self.apply_settings(true);
            }
            Message::SettingsBellModeSelected(mode) => {
                self.settings_draft.bell_mode = mode;
                return self.apply_settings(true);
            }
            Message::SettingsRightClickActionSelected(action) => {
                self.settings_draft.right_click_action = action;
                return self.apply_settings(true);
            }
            Message::FontSelected(option) => {
                self.settings_draft
                    .update(SettingsField::TerminalFontSelection, option.value);
                return self.apply_settings(true);
            }
            Message::ToggleShowAllFonts(show_all) => {
                self.show_all_fonts = show_all;
                self.font_combo_state = super::build_font_combo_state(
                    &self.all_font_options,
                    show_all,
                    self.config.terminal.font_selection.as_deref(),
                );
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
                if self.take_initial_shell_request() {
                    return self.create_tab(ShellKind::Default);
                }
            }
            Message::PtyOutput(event) => {
                self.handle_pty_event(event);
                self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
                return self.sync_terminal_scrollable();
            }
            Message::PtyOutputBatch(events) => {
                for event in events {
                    self.handle_pty_event(event);
                }
                self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
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
            Message::TabBarScrolled(x) => {
                self.tab_bar_scroll_x = x;
            }
            Message::SelectionChanged(sel) => {
                self.selection_autoscroll = None;
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    tab.selection = sel;
                }
            }
            Message::TerminalSelectionAutoscroll { up, col } => {
                self.selection_autoscroll = Some(up);
                self.selection_autoscroll_col = col;
            }
            Message::TerminalSelectionAutoscrollStop => {
                self.selection_autoscroll = None;
            }
            Message::SelectionAutoscrollTick => {
                return self.advance_selection_autoscroll();
            }
            Message::TerminalMousePress { col, row } => {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    tab.send_mouse_event(0, col, row, true);
                }
            }
            Message::TerminalMouseRelease { col, row } => {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    tab.send_mouse_event(0, col, row, false);
                }
            }
            Message::TerminalMouseDrag { col, row } => {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    // Button 0 + 32 = motion flag for SGR drag reporting
                    tab.send_mouse_event(32, col, row, true);
                }
            }
            Message::PasteClipboard(text) => {
                if !text.is_empty() {
                    let is_multiline = text.contains('\n') || text.contains('\r');
                    if self.config.terminal.multiline_paste_confirm && is_multiline {
                        self.pending_paste = Some(text);
                    } else {
                        self.perform_paste(text);
                    }
                }
            }
            Message::ConfirmMultilinePaste => {
                if let Some(text) = self.pending_paste.take() {
                    self.perform_paste(text);
                }
            }
            Message::CancelMultilinePaste => {
                self.pending_paste = None;
            }
            Message::ImeStateChanged(active) => {
                self.ime_active = active;
                if !active {
                    self.ime_preedit = None;
                }
            }
            Message::ImeCommit(text) => {
                if !text.is_empty()
                    && let Some(tab) = self.active_session_mut()
                    && let crate::gui::tab::TerminalSession::Active(session) = &tab.session
                {
                    let _ = session.send_bytes(text.as_bytes());
                    tab.clear_selection();
                }
                self.ime_preedit = None;
                self.scroll_follow_bottom = true;
                self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
                return self.sync_terminal_scrollable();
            }
            Message::ImePreedit(text, cursor) => {
                if text.is_empty() {
                    self.ime_preedit = None;
                } else {
                    self.ime_preedit = Some((text, cursor));
                }
            }
            Message::TerminalScroll(rel_y) => {
                if self.ignore_scrollable_sync > 0 {
                    self.ignore_scrollable_sync -= 1;
                } else if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    // With anchor_bottom: rel_y=0 is bottom, rel_y=1 is top.
                    // scroll_to_relative expects rel=1.0 as bottom, rel=0.0 as top.
                    tab.scroll_to_relative(1.0 - rel_y);
                    let (offset, _) = tab.scroll_position();
                    self.scroll_follow_bottom = offset == 0;
                }
            }
            Message::TerminalWheelScroll(raw_delta) => {
                let raw_delta = raw_delta * self.config.terminal.scroll_multiplier;
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    if tab.mouse_mode() {
                        self.scroll_accumulator += raw_delta;
                        let lines = self.scroll_accumulator as i32;
                        if lines != 0 {
                            self.scroll_accumulator -= lines as f32;
                            let button: u8 = if lines > 0 { 64 } else { 65 };
                            for _ in 0..lines.unsigned_abs() {
                                tab.send_mouse_event(button, 0, 0, true);
                            }
                        }
                    } else if tab.alt_screen() {
                        // Alt screen without mouse mode: convert scroll to arrow keys
                        self.scroll_accumulator += raw_delta;
                        let lines = self.scroll_accumulator as i32;
                        if lines != 0 {
                            self.scroll_accumulator -= lines as f32;
                            tab.send_scroll_as_arrows(lines);
                        }
                    } else {
                        self.scroll_accumulator = 0.0;
                        let delta = raw_delta.round() as i32;
                        if delta != 0 {
                            tab.scroll(delta);
                        }
                        // Update follow-bottom based on resulting position
                        let (offset, _) = tab.scroll_position();
                        self.scroll_follow_bottom = offset == 0;
                    }
                }
                if self
                    .tabs
                    .get(self.active_tab)
                    .is_some_and(|t| !t.mouse_mode() && !t.alt_screen())
                {
                    self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
                    return self.sync_terminal_scrollable_forced();
                }
            }
            Message::WindowResized(size) => {
                return self.handle_window_resized(size);
            }
            Message::AnimationTick => {
                let now = Instant::now();
                if !self.shell_picker_anim.is_animating(now) && !self.shell_picker_anim.value() {
                    self.show_shell_picker = false;
                    self.shell_picker_selected = 0;
                }
                for tab in &mut self.tabs {
                    if !tab.sftp.anim.is_animating(now) && !tab.sftp.anim.value() {
                        tab.sftp.open = false;
                    }
                }
                if let Some(cat) = self.settings_category_transition.tick(now) {
                    self.settings_category = cat;
                }
                if let Some(start) = self.bell_flash_start
                    && start.elapsed() >= super::BELL_FLASH_DURATION
                {
                    self.bell_flash_start = None;
                }
            }
            Message::CursorBlink => {
                self.cursor_blink_on = !self.cursor_blink_on;
            }
            Message::ResizeDebounce => {
                if self.resize_debounce_seq != self.resize_debounce_spawned_seq {
                    // New resizes arrived during the wait -> restart timer
                    self.resize_debounce_spawned_seq = self.resize_debounce_seq;
                    return Task::perform(
                        async {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        },
                        |()| Message::ResizeDebounce,
                    );
                }
                self.resize_debounce_pending = false;
                self.apply_resize();
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
                return iced::window::latest().and_then(|id| iced::window::minimize(id, true));
            }
            #[cfg(target_os = "windows")]
            Message::WindowMaximize => {
                return iced::window::latest().and_then(iced::window::toggle_maximize);
            }
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            Message::WindowDrag => {
                if self.dragging_tab.is_none() {
                    return iced::window::latest().and_then(iced::window::drag);
                }
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
        // The multi-line paste confirmation is the topmost overlay; while it is
        // shown, Enter confirms, Escape cancels, and all other keys are swallowed.
        if self.pending_paste.is_some() {
            match key {
                Key::Named(Named::Enter) => return self.update(Message::ConfirmMultilinePaste),
                Key::Named(Named::Escape) => return self.update(Message::CancelMultilinePaste),
                _ => {}
            }
            return Task::none();
        }

        if self.show_shell_picker {
            match key {
                Key::Named(Named::Escape) => {
                    return self.update(Message::CloseShellPicker);
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

        // Cmd+1..9 (macOS) / Ctrl+1..9 (other) — switch to Nth tab
        if let Key::Character(ref c) = key
            && let Some(digit) = c.chars().next().and_then(|ch| ch.to_digit(10))
        {
            #[cfg(target_os = "macos")]
            let modifier_held = modifiers.logo();
            #[cfg(not(target_os = "macos"))]
            let modifier_held = modifiers.control();

            if modifier_held && (1..=9).contains(&digit) {
                let target = (digit as usize) - 1;
                if target < self.tabs.len() {
                    self.active_tab = target;
                }
                return Task::none();
            }
        }

        if let Some(task) = self.handle_app_shortcut(&key, modifiers) {
            return task;
        }

        if self.active_tab == SETTINGS_TAB_INDEX {
            return Task::none();
        }

        // Copy: Cmd+C (macOS) / Ctrl+Shift+C (other)
        if is_copy_shortcut(&key, modifiers)
            && let Some(tab) = self.tabs.get_mut(self.active_tab)
            && let Some(text) = tab.selected_text()
        {
            tab.clear_selection();
            return iced::clipboard::write(text);
        }
        // No selection → fall through to send Ctrl+C to terminal

        // Paste: Cmd+V (macOS) / Ctrl+Shift+V (other)
        if is_paste_shortcut(&key, modifiers) {
            return iced::clipboard::read()
                .map(|content| Message::PasteClipboard(content.unwrap_or_default()));
        }

        // Ignore modifier-only key presses
        if matches!(
            key,
            Key::Named(
                Named::Super
                    | Named::Control
                    | Named::Shift
                    | Named::Alt
                    | Named::Meta
                    | Named::Hyper
            )
        ) {
            return Task::none();
        }

        if self.ime_preedit.is_some() && matches!(key, Key::Character(_)) && !modifiers.control() {
            return Task::none();
        }

        // Clear selection on actual key input
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.clear_selection();
            tab.handle_key(&key, modifiers, text.as_deref());
        }
        self.scroll_follow_bottom = true;
        self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
        self.sync_terminal_scrollable()
    }

    fn advance_selection_autoscroll(&mut self) -> Task<Message> {
        let Some(up) = self.selection_autoscroll else {
            return Task::none();
        };
        if self.active_tab == SETTINGS_TAB_INDEX {
            self.selection_autoscroll = None;
            return Task::none();
        }
        let col = self.selection_autoscroll_col;
        let step = (self.config.terminal.scroll_multiplier.round() as i32).max(1);
        let offset = {
            let Some(tab) = self.tabs.get_mut(self.active_tab) else {
                self.selection_autoscroll = None;
                return Task::none();
            };
            let Some(mut sel) = tab.selection else {
                self.selection_autoscroll = None;
                return Task::none();
            };
            let lines = tab.size().lines;
            tab.scroll(if up { step } else { -step });
            let (offset, _) = tab.scroll_position();
            let edge_row = if up {
                0i64
            } else {
                lines.saturating_sub(1) as i64
            };
            let delta = offset as i64 - sel.anchor_offset as i64;
            sel.end = crate::terminal::SelectionPoint {
                row: edge_row - delta,
                col,
            };
            tab.selection = Some(sel);
            offset
        };
        self.scroll_follow_bottom = offset == 0;
        self.ignore_scrollable_sync = IGNORE_SCROLL_SYNC_COUNT;
        self.sync_terminal_scrollable_forced()
    }

    fn perform_paste(&mut self, text: String) {
        let config_bracketed_paste = self.config.terminal.bracketed_paste;
        if let Some(tab) = self.active_session_mut()
            && let crate::gui::tab::TerminalSession::Active(session) = &tab.session
        {
            // pasted contents cannot break out of bracketed-paste framing.
            let sanitized = text
                .replace("\r\n", "\r")
                .replace('\n', "\r")
                .replace("\x1b[200~", "")
                .replace("\x1b[201~", "");
            let payload = if config_bracketed_paste && tab.bracketed_paste() {
                format!("\x1b[200~{sanitized}\x1b[201~").into_bytes()
            } else {
                sanitized.into_bytes()
            };
            let _ = session.send_bytes(&payload);
        }
    }

    fn handle_apply_window_style(&mut self) -> Task<Message> {
        if self.window_style_applied {
            return Task::none();
        }
        self.window_style_applied = true;

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            let theme = self.config.theme.clone();
            iced::window::latest()
                .and_then(move |id| {
                    let theme = theme.clone();
                    iced::window::run(id, move |window| {
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

fn is_copy_shortcut(key: &Key, modifiers: iced::keyboard::Modifiers) -> bool {
    if let Key::Character(c) = key
        && c.eq_ignore_ascii_case("c")
    {
        #[cfg(target_os = "macos")]
        return modifiers.logo();
        #[cfg(not(target_os = "macos"))]
        return modifiers.control() && modifiers.shift();
    }
    false
}

fn is_paste_shortcut(key: &Key, modifiers: iced::keyboard::Modifiers) -> bool {
    if let Key::Character(c) = key
        && c.eq_ignore_ascii_case("v")
    {
        #[cfg(target_os = "macos")]
        return modifiers.logo();
        #[cfg(not(target_os = "macos"))]
        return modifiers.control() && modifiers.shift();
    }
    false
}
