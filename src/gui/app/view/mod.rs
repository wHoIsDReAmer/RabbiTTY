mod dialog;
mod settings;
mod shell_picker;

pub(in crate::gui) use dialog::{DialogButton, confirm_dialog};

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::components::{panel, tab_bar};
use crate::gui::render::TerminalProgram;
use iced::widget::{column, container, row, scrollable, stack, text};
use iced::{Element, Length};

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        self.view_main()
    }

    fn view_main(&self) -> Element<'_, Message> {
        let tabs_iter = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.title.as_str(), i, i == self.active_tab));
        let settings_iter = self
            .settings_open
            .then_some((
                "Settings",
                SETTINGS_TAB_INDEX,
                self.active_tab == SETTINGS_TAB_INDEX,
            ))
            .into_iter();
        let tabs_iter = tabs_iter.chain(settings_iter);
        let ui_alpha = self.config.theme.background_opacity;
        let bar_alpha = (ui_alpha * 0.9).clamp(0.0, 1.0);
        let tab_alpha = (ui_alpha * 0.6).clamp(0.0, 1.0);
        let tab_row = tab_bar(
            tabs_iter,
            Message::OpenShellPicker,
            Message::OpenSettingsTab,
            bar_alpha,
            tab_alpha,
        );

        let main_content: Element<Message> = if self.active_tab == SETTINGS_TAB_INDEX {
            self.view_settings()
        } else if let Some(active_tab) = self.tabs.get(self.active_tab) {
            self.view_terminal(active_tab)
        } else {
            column(vec![
                text("No tabs open").size(20).into(),
                text("Click + to create a new tab").size(14).into(),
            ])
            .spacing(8)
            .padding(20)
            .into()
        };

        let panel_background = Some(self.theme_background_color());
        let base_layout = panel(
            column(vec![tab_row, main_content]).height(Length::Fill),
            panel_background,
            self.theme_text_color(),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        #[cfg(target_os = "macos")]
        if self.show_restart_confirm {
            return confirm_dialog(
                base_layout,
                "Blur on macOS requires restart.",
                "Save changes and restart now?",
                vec![
                    DialogButton {
                        label: "Cancel".into(),
                        message: Message::CancelRestartForBlur,
                        primary: false,
                    },
                    DialogButton {
                        label: "Save & Restart".into(),
                        message: Message::ConfirmRestartForBlur,
                        primary: true,
                    },
                ],
                Message::CancelRestartForBlur,
            );
        }

        if self.show_shell_picker {
            return self.view_shell_picker(base_layout);
        }

        base_layout.into()
    }

    fn view_terminal<'a>(
        &'a self,
        active_tab: &'a crate::gui::tab::TerminalTab,
    ) -> Element<'a, Message> {
        let dims = active_tab.size();
        let cells = active_tab.render_cells();
        let grid_size = dims;
        let bg = self.config.theme.background;
        let bg_a = self.config.theme.background_opacity;
        let clear_color = [
            super::srgb_u8_to_linear(bg[0]),
            super::srgb_u8_to_linear(bg[1]),
            super::srgb_u8_to_linear(bg[2]),
            bg_a,
        ];
        let terminal_widget = TerminalProgram {
            cells,
            grid_size,
            terminal_font_selection: self.config.terminal.font_selection.clone(),
            terminal_font_size: self.config.terminal.font_size,
            padding: [
                self.config.terminal.padding_x,
                self.config.terminal.padding_y,
            ],
            clear_color,
        }
        .widget()
        .width(Length::Fill)
        .height(Length::Fill);

        let (_scroll_offset, scroll_history) = active_tab.scroll_position();
        if scroll_history > 0 {
            let cell_height = self.config.terminal.cell_height.max(1.0);
            let content_height = (scroll_history + dims.lines) as f32 * cell_height;
            let scroll_content = container("")
                .width(Length::Fill)
                .height(Length::Fixed(content_height));

            let scroll_overlay = scrollable(scroll_content)
                .id(crate::gui::app::update::TERMINAL_SCROLLABLE_ID.clone())
                .direction(scrollable::Direction::Vertical(
                    scrollable::Scrollbar::new().width(8).scroller_width(8),
                ))
                .on_scroll(|viewport: scrollable::Viewport| {
                    let rel = viewport.relative_offset();
                    Message::TerminalScroll(rel.y)
                })
                .style(crate::gui::theme::scrollbar_style)
                .width(Length::Fixed(14.0))
                .height(Length::Fill);

            stack![
                terminal_widget,
                row![container("").width(Length::Fill), scroll_overlay].height(Length::Fill)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            terminal_widget.into()
        }
    }
}
