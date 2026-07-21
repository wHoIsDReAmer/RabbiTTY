mod dialog;
mod password_prompt;
mod settings;
mod sftp;
mod shell_picker;

pub(in crate::gui) use dialog::{DialogButton, confirm_dialog};

use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::config::TabBarPosition;
use crate::gui::app::{SettingsMessage, SftpMessage};
use crate::gui::components::context_menu::{ContextMenuItem, context_menu};
use crate::gui::components::ime_wrapper::ImeEnabled;
use crate::gui::components::{panel, secondary as button_secondary, tab_bar};
use crate::gui::render::TerminalProgram;
use crate::gui::theme::{RADIUS_SMALL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL};
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
        // Faint tint over the translucent base so blur shows through the bar.
        let bar_alpha = 0.22;
        let tab_alpha = (ui_alpha * 0.6).clamp(0.0, 1.0);
        let sftp_toggle = if self.active_tab != SETTINGS_TAB_INDEX {
            self.tabs.get(self.active_tab).and_then(|tab| {
                tab.profile
                    .ssh_profile()
                    .is_some()
                    .then_some((Message::Sftp(SftpMessage::ToggleDrawer), tab.sftp.open))
            })
        } else {
            None
        };
        let tab_row = tab_bar(
            tabs_iter,
            Message::OpenShellPicker,
            Message::Settings(SettingsMessage::OpenTab),
            sftp_toggle,
            bar_alpha,
            tab_alpha,
            self.dragging_tab,
            self.drag_target,
            palette,
            self.config.ui.animations_enabled,
            self.config.ui.tab_bar_position,
        );

        let main_content: Element<Message> = if self.active_tab == SETTINGS_TAB_INDEX {
            self.view_settings()
        } else if let Some(active_tab) = self.tabs.get(self.active_tab) {
            self.view_terminal(active_tab)
        } else {
            self.view_lobby(palette)
        };

        let layout = match self.config.ui.tab_bar_position {
            TabBarPosition::Top => column(vec![tab_row, main_content]),
            TabBarPosition::Bottom => column(vec![
                crate::gui::components::tab_bar::window_chrome(palette, bar_alpha),
                main_content,
                tab_row,
            ]),
        };

        // `window_style` already clears the window to `background @ opacity`.
        let panel_background = None;
        let base_layout = panel(
            layout.height(Length::Fill),
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
                        message: Message::Settings(SettingsMessage::CancelRestartForBlur),
                        primary: false,
                    },
                    DialogButton {
                        label: "Save & Restart".into(),
                        message: Message::Settings(SettingsMessage::ConfirmRestartForBlur),
                        primary: true,
                    },
                ],
                Message::Settings(SettingsMessage::CancelRestartForBlur),
                palette,
                self.config.ui.animations_enabled,
            );
        }

        if let Some(text) = self.pending_paste.as_deref() {
            let line_count = text.lines().count().max(1);
            let description =
                t!("dialog.paste_multiline_body").replace("{count}", &line_count.to_string());
            return confirm_dialog(
                base_layout,
                t!("dialog.paste_multiline_title"),
                &description,
                vec![
                    DialogButton {
                        label: t!("dialog.cancel").into(),
                        message: Message::CancelMultilinePaste,
                        primary: false,
                    },
                    DialogButton {
                        label: t!("dialog.paste").into(),
                        message: Message::ConfirmMultilinePaste,
                        primary: true,
                    },
                ],
                Message::CancelMultilinePaste,
                palette,
                self.config.ui.animations_enabled,
            );
        }

        if let Some(prompt) = self.password_prompt.as_ref() {
            return password_prompt::password_prompt(base_layout, prompt, palette);
        }

        if self.show_shell_picker {
            return self.view_shell_picker(base_layout);
        }

        if let Some(tab_index) = self.tab_context_menu {
            return self.view_tab_context_menu(base_layout, tab_index);
        }

        if self.terminal_context_menu {
            return self.view_terminal_context_menu(base_layout);
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
        let cursor = active_tab
            .cursor_cell()
            .map(|(col, row)| [col as u32, row as u32]);
        let cursor_visible = !self.config.terminal.cursor_blink || self.cursor_blink_on;
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
            cursor,
            cursor_shape: self.config.terminal.cursor_shape,
            cursor_visible,
            cursor_color: active_tab.cursor_color(),
            background_opacity: self.config.theme.background_opacity,
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

            let scrollbar = scrollable(scroll_content)
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
                .width(Length::Fixed(8.0))
                .height(Length::Fill);

            row![terminal_widget, scrollbar]
                .height(Length::Fill)
                .width(Length::Fill)
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
            let drawer_panel = container(sftp::drawer(
                &active_tab.sftp,
                active_tab.id,
                self.palette,
                self.config.ui.animations_enabled,
            ))
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

        // Visual bell: a translucent flash that fades out over its duration.
        let with_flash: Element<Message> = if let Some(progress) = self
            .bell_flash_start
            .map(|start| start.elapsed())
            .filter(|elapsed| *elapsed < super::BELL_FLASH_DURATION)
            .map(|elapsed| elapsed.as_secs_f32() / super::BELL_FLASH_DURATION.as_secs_f32())
        {
            let alpha = 0.5 * (1.0 - progress).clamp(0.0, 1.0);
            let flash_color = Color {
                a: alpha,
                ..self.theme_text_color()
            };
            let flash = container("")
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(flash_color)),
                    ..Default::default()
                });
            stack![with_drawer, flash]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            with_drawer
        };

        let (cursor_col, cursor_row) = active_tab.cursor_position();
        let cursor_cell = crate::gui::components::ime_wrapper::CursorCell {
            col: cursor_col,
            row: cursor_row,
            grid_cols: dims.columns,
            grid_rows: dims.lines,
            padding: [
                self.config.terminal.padding_x,
                self.config.terminal.padding_y,
            ],
        };

        ImeEnabled::new(with_flash)
            .preedit(self.ime_preedit.clone())
            .cursor_cell(Some(cursor_cell))
            .text_size(self.config.terminal.font_size)
            .into()
    }

    fn view_lobby(&self, palette: crate::gui::theme::Palette) -> Element<'_, Message> {
        let logo = image(LOGO_HANDLE.clone())
            .width(Length::Fixed(112.0))
            .height(Length::Fixed(112.0));
        let title = text("Rabbitty").size(28).color(palette.text);
        let version_label = text(format!("v{}", env!("CARGO_PKG_VERSION")))
            .size(12)
            .color(Color {
                a: 0.45,
                ..palette.text_secondary
            });
        let header = column![logo, title, version_label]
            .spacing(SPACING_SMALL)
            .align_x(Alignment::Center);
        let new_tab_btn = button_secondary(
            t!("lobby.new_tab"),
            Some(Message::OpenShellPicker),
            palette,
            self.config.ui.animations_enabled,
        );

        let mut content: Vec<Element<Message>> = vec![header.into(), new_tab_btn];

        if !self.session_history.entries.is_empty() {
            let divider = container(text(""))
                .width(Length::Fixed(240.0))
                .height(1)
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.1,
                        ..palette.text
                    })),
                    ..Default::default()
                });
            let recent_label = text(t!("lobby.recent_sessions")).size(11).color(Color {
                a: 0.5,
                ..palette.text_secondary
            });
            let mut session_items: Vec<Element<Message>> = Vec::new();

            for (i, entry) in self.session_history.entries.iter().enumerate() {
                let name = entry.display_name.clone();
                let kind_label = match &entry.profile.kind {
                    crate::gui::tab::ProfileKind::Local { program: None, .. } => {
                        t!("session_kind.default_shell")
                    }
                    crate::gui::tab::ProfileKind::Local { .. } => t!("session_kind.shell"),
                    crate::gui::tab::ProfileKind::Ssh(_) => t!("session_kind.ssh"),
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

                // The hover background is painted by `hover_fade`; the button
                // itself stays transparent.
                let session_btn = button(label_col)
                    .style(
                        move |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                            iced::widget::button::Style {
                                background: Some(Background::Color(Color::TRANSPARENT)),
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
                    .on_press(Message::LaunchFromHistory(i));

                let rest = crate::gui::components::HoverStyle {
                    background: Color::TRANSPARENT,
                    border_color: Color::TRANSPARENT,
                    border_width: 0.0,
                    radius: RADIUS_SMALL,
                };
                let hover = crate::gui::components::HoverStyle {
                    background: Color {
                        a: 0.08,
                        ..palette.text
                    },
                    ..rest
                };

                session_items.push(
                    crate::gui::components::hover_fade(
                        session_btn,
                        rest,
                        hover,
                        self.config.ui.animations_enabled,
                    )
                    .into(),
                );
            }

            let recent = column![
                divider,
                recent_label,
                column(session_items).spacing(2).align_x(Alignment::Center),
            ]
            .spacing(SPACING_NORMAL)
            .align_x(Alignment::Center);
            content.push(recent.into());
        }

        container(
            column(content)
                .spacing(SPACING_LARGE)
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
            self.config.ui.animations_enabled,
        )
    }

    fn view_terminal_context_menu<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let has_selection = self
            .tabs
            .get(self.active_tab)
            .and_then(|tab| tab.selected_text())
            .is_some();

        let mut items = Vec::new();
        if has_selection {
            items.push(ContextMenuItem {
                label: t!("context_menu.copy"),
                message: Message::TerminalContextCopy,
            });
        }
        items.push(ContextMenuItem {
            label: t!("context_menu.paste"),
            message: Message::TerminalContextPaste,
        });

        context_menu(
            base_layout,
            items,
            self.cursor_position,
            Message::CloseTerminalContextMenu,
            self.palette,
            self.config.ui.animations_enabled,
        )
    }
}
