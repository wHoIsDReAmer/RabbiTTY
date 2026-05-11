use super::super::{App, Message, SETTINGS_TAB_INDEX};
use super::{TAB_BAR_SCROLLABLE_ID, TERMINAL_SCROLLABLE_ID};
use crate::config::AppConfigUpdates;
use crate::session::OutputEvent;
use iced::widget::operation::{scroll_to, snap_to};
use iced::widget::scrollable;
use iced::{Size, Task};

impl App {
    pub(super) fn handle_pty_event(&mut self, event: OutputEvent) {
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

    pub(super) fn sync_terminal_scrollable_forced(&self) -> Task<Message> {
        if self.active_tab == SETTINGS_TAB_INDEX {
            return Task::none();
        }
        let Some(tab) = self.tabs.get(self.active_tab) else {
            return Task::none();
        };
        let (offset, history) = tab.scroll_position();
        if history == 0 {
            return Task::none();
        }
        // With anchor_bottom: rel_y=0 is bottom, rel_y=1 is top
        let rel_y = (offset as f32 / history as f32).clamp(0.0, 1.0);
        snap_to(
            TERMINAL_SCROLLABLE_ID.clone(),
            scrollable::RelativeOffset { x: 0.0, y: rel_y },
        )
    }

    pub(super) fn sync_terminal_scrollable(&self) -> Task<Message> {
        if self.active_tab == SETTINGS_TAB_INDEX {
            return Task::none();
        }

        let Some(tab) = self.tabs.get(self.active_tab) else {
            return Task::none();
        };

        let (_offset, history) = tab.scroll_position();
        if history == 0 {
            return Task::none();
        }

        // No explicit snap needed.
        Task::none()
    }

    pub(super) fn handle_tab_bar_scroll(&mut self, delta: f32) -> Task<Message> {
        let new_x = (self.tab_bar_scroll_x - delta).max(0.0);
        self.tab_bar_scroll_x = new_x;
        scroll_to(
            TAB_BAR_SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset { x: new_x, y: 0.0 },
        )
    }

    pub(super) fn handle_window_resized(&mut self, size: Size) -> Task<Message> {
        self.window_size = size;
        self.resize_debounce_seq += 1;

        if self.resize_debounce_pending {
            return Task::none();
        }

        self.resize_debounce_pending = true;
        self.resize_debounce_spawned_seq = self.resize_debounce_seq;
        Task::perform(
            async {
                std::thread::sleep(std::time::Duration::from_millis(50));
            },
            |()| Message::ResizeDebounce,
        )
    }

    pub(super) fn apply_resize(&mut self) {
        let size = self.window_size;

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
        if (self.config.ui.window_width - previous_width).abs() > f32::EPSILON
            || (self.config.ui.window_height - previous_height).abs() > f32::EPSILON
        {
            self.queue_config_save();
        }

        let (cols, rows) = self.grid_for_size(size);

        for tab in &mut self.tabs {
            tab.resize(cols, rows);
        }
    }
}
