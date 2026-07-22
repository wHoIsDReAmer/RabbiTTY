mod settings;
mod sftp;
pub(in crate::gui) mod tab;
mod terminal;

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::settings::SettingsDraft;
use crate::gui::tab::Profile;
use iced::keyboard::{Key, key::Named};
use iced::time::Instant;
use iced::{Task, widget};
use std::sync::LazyLock;

pub(in crate::gui) static TAB_BAR_SCROLLABLE_ID: LazyLock<widget::Id> =
    LazyLock::new(widget::Id::unique);

const WHEEL_GESTURE_IDLE: std::time::Duration = std::time::Duration::from_millis(100);

fn is_gesture_tail(suppressed: bool, gap: Option<std::time::Duration>) -> bool {
    suppressed && gap.is_some_and(|gap| gap <= WHEEL_GESTURE_IDLE)
}

impl App {
    fn active_session_mut(&mut self) -> Option<&mut crate::gui::tab::Pane> {
        self.focused_pane_mut()
            .filter(|pane| matches!(pane.session, crate::gui::tab::TerminalSession::Active(_)))
    }

    pub(super) fn dismiss_shell_picker(&mut self) {
        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        self.modal_anim.go_mut(false, Instant::now());
    }

    fn save_profiles(&mut self) {
        self.config.profiles = self.settings_draft.collect_profiles();

        match self.config.save() {
            Ok(()) => {
                self.settings_draft = SettingsDraft::from_config(&self.config);
                self.settings_draft.set_profiles_saved();
            }
            Err(err) => {
                let message = format!("Failed to save profiles: {err}");
                eprintln!("{message}");
                self.settings_draft.set_profiles_error(message);
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
                self.modal_anim.go_mut(true, Instant::now());
            }
            Message::CloseShellPicker => {
                self.modal_anim.go_mut(false, Instant::now());
            }
            Message::CreateTab(profile) => return self.launch_profile(profile),
            Message::LaunchFromHistory(index) => {
                if let Some(entry) = self.session_history.entries.get(index).cloned() {
                    return self.launch_profile(entry.profile);
                }
            }
            Message::DuplicateTab => {
                let index = self.tab_context_menu.unwrap_or(self.active_tab);
                self.tab_context_menu = None;
                if let Some(tab) = self.tabs.get(index) {
                    let profile = tab.focused().profile.clone();
                    return self.launch_profile(profile);
                }
            }
            Message::Sftp(message) => return self.update_sftp(message),
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
                    return self.create_tab(Profile::ssh(profile));
                }
            }
            Message::SshPasswordPromptCancel => {
                self.password_prompt = None;
            }
            Message::ShowTabContextMenu(index) => {
                self.tab_context_menu = Some(index);
            }
            Message::CloseTabContextMenu => {
                self.tab_context_menu = None;
            }
            Message::TerminalRightClick(pane) => {
                self.focus_pane(pane);
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
            Message::OpenUrl(url) => {
                crate::platform::open_url(&url);
            }
            Message::TerminalContextCopy => {
                self.terminal_context_menu = false;
                if let Some(pane) = self.focused_pane_mut()
                    && let Some(text) = pane.selected_text()
                {
                    pane.clear_selection();
                    return iced::clipboard::write(text);
                }
            }
            Message::CursorMoved(point) => {
                if self.tab_context_menu.is_none() && !self.terminal_context_menu {
                    self.cursor_position = point;
                }
            }

            // ── Settings ────────────────────────────────────────────
            Message::Settings(message) => return self.update_settings_message(message),

            // ── Terminal / PTY ──────────────────────────────────────
            Message::PtySenderReady(sender) => {
                self.pty_sender = Some(sender);
                if self.take_initial_shell_request() {
                    return self.create_tab(Profile::default_shell());
                }
            }
            Message::PtyOutput(event) => {
                self.handle_pty_event(event);
                return Task::none();
            }
            Message::PtyOutputBatch(events) => {
                for event in events {
                    self.handle_pty_event(event);
                }
                return Task::none();
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
            Message::SelectionChanged { pane, selection } => {
                self.selection_autoscroll = None;
                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(tab) = self.tabs.get_mut(self.active_tab)
                {
                    tab.focused = pane;
                    if let Some(slot) = tab.pane_mut(pane) {
                        slot.selection = selection;
                    }
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
            Message::TerminalMousePress { pane, col, row } => {
                self.focus_pane(pane);
                if let Some(pane) = self.focused_pane() {
                    pane.send_mouse_event(0, col, row, true);
                }
            }
            Message::TerminalMouseRelease { col, row } => {
                if let Some(pane) = self.focused_pane() {
                    pane.send_mouse_event(0, col, row, false);
                }
            }
            Message::TerminalMouseDrag { col, row } => {
                if let Some(pane) = self.focused_pane() {
                    // Button 0 + 32 = motion flag for SGR drag reporting
                    pane.send_mouse_event(32, col, row, true);
                }
            }
            Message::PasteClipboard(text) => {
                if !text.is_empty() {
                    let is_multiline = text.contains('\n') || text.contains('\r');
                    if self.config.terminal.multiline_paste_confirm && is_multiline {
                        self.pending_paste = Some(text);
                    } else {
                        return self.perform_paste(text);
                    }
                }
            }
            Message::ConfirmMultilinePaste => {
                if let Some(text) = self.pending_paste.take() {
                    return self.perform_paste(text);
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
                    && let Some(pane) = self.active_session_mut()
                    && let crate::gui::tab::TerminalSession::Active(session) = &pane.session
                {
                    let _ = session.send_bytes(text.as_bytes());
                    pane.clear_selection();
                    pane.scroll_to_bottom();
                }
                self.ime_preedit = None;
                self.scroll_follow_bottom = true;
                self.wheel_suppressed = true;
                return Task::none();
            }
            Message::ImePreedit(text, cursor) => {
                if text.is_empty() {
                    self.ime_preedit = None;
                } else {
                    self.ime_preedit = Some((text, cursor));
                }
            }
            Message::PaneScrollTo { pane, rel } => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let Some(pane) = tab.pane_mut(pane)
                {
                    pane.scroll_to_relative(rel);
                }
            }
            Message::TerminalWheelScroll(raw_delta) => {
                let now = std::time::Instant::now();
                let gap = self.wheel_last_event.map(|last| now.duration_since(last));
                self.wheel_last_event = Some(now);
                if is_gesture_tail(self.wheel_suppressed, gap) {
                    return Task::none();
                }
                self.wheel_suppressed = false;

                let raw_delta = raw_delta * self.config.terminal.scroll_multiplier;
                let mut accumulator = self.scroll_accumulator;
                let mut follow_bottom = self.scroll_follow_bottom;
                let mut sync = false;

                if self.active_tab != SETTINGS_TAB_INDEX
                    && let Some(pane) = self.focused_pane_mut()
                {
                    if pane.mouse_mode() {
                        accumulator += raw_delta;
                        let lines = accumulator as i32;
                        if lines != 0 {
                            accumulator -= lines as f32;
                            let button: u8 = if lines > 0 { 64 } else { 65 };
                            for _ in 0..lines.unsigned_abs() {
                                pane.send_mouse_event(button, 0, 0, true);
                            }
                        }
                    } else if pane.alt_screen() {
                        accumulator += raw_delta;
                        let lines = accumulator as i32;
                        if lines != 0 {
                            accumulator -= lines as f32;
                            pane.send_scroll_as_arrows(lines);
                        }
                    } else {
                        accumulator = 0.0;
                        let delta = raw_delta.round() as i32;
                        if delta != 0 {
                            pane.scroll(delta);
                            let (offset, _) = pane.scroll_position();
                            follow_bottom = offset == 0;
                            sync = true;
                        }
                    }
                }

                self.scroll_accumulator = accumulator;
                self.scroll_follow_bottom = follow_bottom;
                if sync {
                    return Task::none();
                }
            }
            Message::WindowResized(size) => {
                return self.handle_window_resized(size);
            }
            Message::AnimationTick => {
                let now = Instant::now();
                if !self.modal_anim.is_animating(now) && !self.modal_anim.value() {
                    self.show_shell_picker = false;
                    self.shell_picker_selected = 0;
                }
                for pane in self.panes_mut() {
                    if !pane.sftp.anim.is_animating(now) && !pane.sftp.anim.value() {
                        pane.sftp.open = false;
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
            Message::TerminalAreaResized(size) => {
                if (self.terminal_area.width - size.width).abs() > 0.5
                    || (self.terminal_area.height - size.height).abs() > 0.5
                {
                    self.terminal_area = size;
                    self.resize_panes();
                }
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

        if self.show_shell_picker && self.modal_anim.value() {
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
            && let Some(pane) = self.focused_pane_mut()
            && let Some(text) = pane.selected_text()
        {
            pane.clear_selection();
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
        if let Some(pane) = self.focused_pane_mut() {
            pane.clear_selection();
            pane.handle_key(&key, modifiers, text.as_deref());
            pane.scroll_to_bottom();
        }
        self.scroll_follow_bottom = true;
        self.wheel_suppressed = true;
        Task::none()
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
            let Some(pane) = self.focused_pane_mut() else {
                self.selection_autoscroll = None;
                return Task::none();
            };
            let Some(mut sel) = pane.selection else {
                self.selection_autoscroll = None;
                return Task::none();
            };
            let lines = pane.size().lines;
            pane.scroll(if up { step } else { -step });
            let (offset, _) = pane.scroll_position();
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
            pane.selection = Some(sel);
            offset
        };
        self.scroll_follow_bottom = offset == 0;
        Task::none()
    }

    fn perform_paste(&mut self, text: String) -> Task<Message> {
        let config_bracketed_paste = self.config.terminal.bracketed_paste;
        if let Some(pane) = self.active_session_mut()
            && let crate::gui::tab::TerminalSession::Active(session) = &pane.session
        {
            // pasted contents cannot break out of bracketed-paste framing.
            let sanitized = text
                .replace("\r\n", "\r")
                .replace('\n', "\r")
                .replace("\x1b[200~", "")
                .replace("\x1b[201~", "");
            let payload = if config_bracketed_paste && pane.bracketed_paste() {
                format!("\x1b[200~{sanitized}\x1b[201~").into_bytes()
            } else {
                sanitized.into_bytes()
            };
            let _ = session.send_bytes(&payload);
            pane.scroll_to_bottom();
        }
        self.scroll_follow_bottom = true;
        self.wheel_suppressed = true;
        Task::none()
    }

    fn handle_apply_window_style(&mut self) -> Task<Message> {
        if self.window_style_applied {
            return Task::none();
        }
        self.window_style_applied = true;

        let theme = self.config.theme.clone();
        iced::window::latest()
            .and_then(move |id| {
                let theme = theme.clone();
                iced::window::run(id, move |window| {
                    if let (Ok(window_handle), Ok(display_handle)) =
                        (window.window_handle(), window.display_handle())
                    {
                        crate::platform::apply_style(window_handle, display_handle, &theme);
                    }
                })
            })
            .discard()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn typing_swallows_the_tail_of_an_in_flight_scroll() {
        assert!(is_gesture_tail(true, Some(Duration::from_millis(16))));
    }

    #[test]
    fn a_fresh_gesture_after_typing_still_scrolls() {
        assert!(!is_gesture_tail(true, Some(Duration::from_millis(400))));
        assert!(!is_gesture_tail(true, None));
    }

    #[test]
    fn scrolling_is_untouched_without_typing() {
        assert!(!is_gesture_tail(false, Some(Duration::from_millis(16))));
    }
}
