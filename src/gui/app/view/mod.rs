#[cfg(target_os = "macos")]
mod dialog;
mod settings;
mod sftp;
mod shell_picker;

#[cfg(target_os = "macos")]
pub(in crate::gui) use dialog::{DialogButton, confirm_dialog};

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::components::context_menu::{ContextMenuItem, context_menu};
use crate::gui::components::ime_wrapper::ImeEnabled;
use crate::gui::components::{button_secondary, panel, tab_bar};
use crate::gui::render::TerminalProgram;
use crate::gui::theme::{RADIUS_SMALL, SPACING_SMALL};
use iced::widget::{button, column, container, image, row, scrollable, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Length};
use std::sync::LazyLock;

static LOGO_HANDLE: LazyLock<image::Handle> =
    LazyLock::new(|| image::Handle::from_bytes(&include_bytes!("../../../../assets/logo.png")[..]));

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        self.view_main()
    }

    fn view_main(&self) -> Element<'_, Message> {
        let palette = self.palette;
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
        let sftp_toggle = if self.active_tab != SETTINGS_TAB_INDEX {
            self.tabs.get(self.active_tab).and_then(|tab| {
                matches!(tab.shell, crate::gui::tab::ShellKind::Ssh(_))
                    .then_some((Message::SftpToggleDrawer, tab.sftp.open))
            })
        } else {
            None
        };
        let tab_row = tab_bar(
            tabs_iter,
            Message::OpenShellPicker,
            Message::OpenSettingsTab,
            sftp_toggle,
            bar_alpha,
            tab_alpha,
            self.dragging_tab,
            self.drag_target,
            palette,
        );

        let main_content: Element<Message> = if self.active_tab == SETTINGS_TAB_INDEX {
            self.view_settings()
        } else if let Some(active_tab) = self.tabs.get(self.active_tab) {
            self.view_terminal(active_tab)
        } else {
            self.view_lobby(palette)
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
                palette,
            );
        }

        if self.show_shell_picker {
            return self.view_shell_picker(base_layout);
        }

        if let Some(tab_index) = self.tab_context_menu {
            return self.view_tab_context_menu(base_layout, tab_index);
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

        // identical to other panes (e.g. Settings) and avoids double blending.
        let clear_color = [0.0, 0.0, 0.0, 0.0];
        let (display_offset, scroll_history) = active_tab.scroll_position();
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
            selection: active_tab.selection,
            mouse_mode: active_tab.mouse_mode(),
            display_offset,
        }
        .widget()
        .width(Length::Fill)
        .height(Length::Fill);

        let terminal_view: Element<Message> = if scroll_history > 0 {
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
                .anchor_bottom()
                .on_scroll(|viewport: scrollable::Viewport| {
                    let rel = viewport.relative_offset();
                    Message::TerminalScroll(rel.y)
                })
                .style(crate::gui::theme::scrollbar_style(self.palette))
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
        };

        let now = iced::time::Instant::now();
        let drawer_progress: f32 = active_tab
            .sftp
            .anim
            .interpolate(0.0f32, 1.0f32, now)
            .clamp(0.0, 1.0);
        let drawer_visible = active_tab.sftp.open || drawer_progress > 0.001;
        let with_drawer: Element<Message> = if drawer_visible {
            let height_ratio = active_tab.sftp.height_ratio.clamp(0.15, 0.85);
            let effective = (height_ratio * drawer_progress).clamp(0.0, height_ratio);
            let bottom_portion = ((effective * 1000.0).round() as u16).max(1);
            let top_portion = 1000u16.saturating_sub(bottom_portion).max(1);
            let drawer_panel =
                container(sftp::drawer(&active_tab.sftp, active_tab.id, self.palette))
                    .width(Length::Fill)
                    .height(Length::FillPortion(bottom_portion))
                    .clip(true);
            let overlay = column![
                iced::widget::Space::new().height(Length::FillPortion(top_portion)),
                drawer_panel,
            ]
            .width(Length::Fill)
            .height(Length::Fill);
            stack![terminal_view, overlay]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            terminal_view
        };

        ImeEnabled::new(with_drawer)
            .preedit(self.ime_preedit.clone())
            .into()
    }

    fn view_lobby(&self, palette: crate::gui::theme::Palette) -> Element<'_, Message> {
        let logo = image(LOGO_HANDLE.clone())
            .width(Length::Fixed(96.0))
            .height(Length::Fixed(96.0));
        let version_label = text(format!("RabbiTTY v{}", env!("CARGO_PKG_VERSION")))
            .size(13)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.4));
        let new_tab_btn =
            button_secondary(t!("lobby.new_tab"), palette).on_press(Message::OpenShellPicker);

        let mut content: Vec<Element<Message>> =
            vec![logo.into(), version_label.into(), new_tab_btn.into()];

        if !self.session_history.entries.is_empty() {
            content.push(
                container(text(""))
                    .width(Length::Fixed(240.0))
                    .height(1)
                    .style(move |_theme: &iced::Theme| container::Style {
                        background: Some(Background::Color(Color {
                            a: 0.1,
                            ..palette.text
                        })),
                        ..Default::default()
                    })
                    .into(),
            );
            content.push(
                text(t!("lobby.recent_sessions"))
                    .size(11)
                    .color(Color {
                        a: 0.5,
                        ..palette.text_secondary
                    })
                    .into(),
            );

            for (i, entry) in self.session_history.entries.iter().enumerate() {
                let name = entry.display_name.clone();
                let kind_label = match &entry.kind {
                    crate::session::history::SessionKind::Default => {
                        t!("session_kind.default_shell")
                    }
                    crate::session::history::SessionKind::Shell { .. } => t!("session_kind.shell"),
                    crate::session::history::SessionKind::Ssh { .. } => t!("session_kind.ssh"),
                };

                let label_col = column![
                    text(name).size(13).color(Color {
                        a: 0.9,
                        ..palette.text
                    }),
                    text(kind_label).size(10).color(Color {
                        a: 0.5,
                        ..palette.text_secondary
                    }),
                ]
                .spacing(1);

                content.push(
                    button(label_col)
                        .style(
                            move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                                let hovered =
                                    matches!(status, iced::widget::button::Status::Hovered);
                                iced::widget::button::Style {
                                    background: Some(Background::Color(if hovered {
                                        Color {
                                            a: 0.08,
                                            ..palette.text
                                        }
                                    } else {
                                        Color::TRANSPARENT
                                    })),
                                    text_color: palette.text,
                                    border: Border {
                                        radius: RADIUS_SMALL.into(),
                                        width: 0.0,
                                        color: Color::TRANSPARENT,
                                    },
                                    shadow: iced::Shadow::default(),
                                    snap: false,
                                }
                            },
                        )
                        .padding([6, 10])
                        .width(Length::Fixed(240.0))
                        .on_press(Message::LaunchFromHistory(i))
                        .into(),
                );
            }
        }

        container(
            column(content)
                .spacing(SPACING_SMALL)
                .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into()
    }

    fn view_tab_context_menu<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
        tab_index: usize,
    ) -> Element<'a, Message> {
        context_menu(
            base_layout,
            vec![
                ContextMenuItem {
                    label: t!("context_menu.duplicate"),
                    message: Message::DuplicateTab,
                },
                ContextMenuItem {
                    label: t!("context_menu.close"),
                    message: Message::CloseTab(tab_index),
                },
            ],
            self.cursor_position,
            Message::CloseTabContextMenu,
            self.palette,
        )
    }
}
