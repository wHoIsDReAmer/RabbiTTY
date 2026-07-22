use super::super::{App, Message, SETTINGS_TAB_INDEX};
use super::TAB_BAR_SCROLLABLE_ID;
use crate::config::{AppConfigUpdates, BellMode};
use crate::session::OutputEvent;
use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Size, Task};

impl App {
    pub(super) fn handle_pty_event(&mut self, event: OutputEvent) {
        match event {
            OutputEvent::Data { tab_id, bytes } => {
                if let Some(pane) = self.pane_mut_by_id(tab_id) {
                    let bell = pane.feed_bytes(&bytes);
                    if bell {
                        self.handle_bell(tab_id);
                    }
                }
            }
            OutputEvent::Closed { tab_id } => {
                if let Some(index) = self
                    .tabs
                    .iter()
                    .position(|t| t.panes.iter().any(|p| p.id == tab_id))
                {
                    let closed_tab = {
                        let tab = &mut self.tabs[index];
                        tab.focused = tab_id;
                        !tab.close_focused()
                    };
                    if closed_tab {
                        self.tabs.remove(index);
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                    }
                }
            }
        }
    }

    /// Reacts to a terminal bell from the tab identified by `tab_id`,
    /// according to the configured bell mode.
    fn handle_bell(&mut self, tab_id: u64) {
        match self.config.terminal.bell_mode {
            BellMode::Off => {}
            BellMode::Sound => crate::platform::ring_bell(),
            BellMode::Visual => {
                let is_active = self.active_tab != SETTINGS_TAB_INDEX
                    && self
                        .tabs
                        .get(self.active_tab)
                        .is_some_and(|t| t.id == tab_id);
                if is_active {
                    self.bell_flash_start = Some(std::time::Instant::now());
                }
            }
        }
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
        if (self.config.ui.window_width - previous_width).abs() > f32::EPSILON
            || (self.config.ui.window_height - previous_height).abs() > f32::EPSILON
        {
            self.queue_config_save();
        }

        self.resize_panes();
    }
}
