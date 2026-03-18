use super::super::{App, Message, SETTINGS_TAB_INDEX};
use super::{TAB_BAR_SCROLLABLE_ID, TERMINAL_SCROLLABLE_ID};
use crate::config::AppConfigUpdates;
use crate::session::OutputEvent;
use iced::widget::operation::scroll_to;
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
        let rel_y = 1.0 - (offset as f32 / history as f32).clamp(0.0, 1.0);
        let content_height = history as f32 * self.config.terminal.cell_height.max(1.0);
        scroll_to(
            TERMINAL_SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: rel_y * content_height,
            },
        )
    }

    pub(super) fn sync_terminal_scrollable(&self) -> Task<Message> {
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
        if offset > 0 {
            return Task::none();
        }
        let content_height = history as f32 * self.config.terminal.cell_height.max(1.0);
        scroll_to(
            TERMINAL_SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: content_height,
            },
        )
    }

    pub(super) fn handle_tab_bar_scroll(&mut self, delta: f32) -> Task<Message> {
        let tab_count = self.tabs.len() + if self.settings_open { 1 } else { 0 };
        let max_offset = (tab_count as f32 * 150.0).max(0.0);

        self.tab_bar_scroll_offset = (self.tab_bar_scroll_offset + delta).clamp(0.0, max_offset);

        scroll_to(
            TAB_BAR_SCROLLABLE_ID.clone(),
            scrollable::AbsoluteOffset {
                x: self.tab_bar_scroll_offset,
                y: 0.0,
            },
        )
    }

    pub(super) fn handle_window_resized(&mut self, size: Size) {
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
