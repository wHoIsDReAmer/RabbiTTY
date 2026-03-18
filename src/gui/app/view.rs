use super::{App, Message, SETTINGS_TAB_INDEX};
use crate::gui::components::{button_primary, button_secondary, panel, tab_bar};
use crate::gui::render::TerminalProgram;
use crate::gui::settings::{self, SettingsCategory};
use crate::gui::tab::ShellKind;
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{button, center, column, container, mouse_area, row, scrollable, stack, text};
use iced::{Background, Border, Color, Element, Length};

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
            self.view_config()
        } else if let Some(active_tab) = self.tabs.get(self.active_tab) {
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

            terminal_widget.into()
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
            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.4,
                        })),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CancelRestartForBlur);

            let popup_card = container(
                column(vec![
                    text("Blur on macOS requires restart.").size(16).into(),
                    text("Save changes and restart now?").size(13).into(),
                    row(vec![
                        button_secondary("Cancel")
                            .on_press(Message::CancelRestartForBlur)
                            .into(),
                        button_primary("Save & Restart")
                            .on_press(Message::ConfirmRestartForBlur)
                            .into(),
                    ])
                    .spacing(SPACING_SMALL)
                    .into(),
                ])
                .spacing(SPACING_NORMAL)
                .padding(20)
                .width(Length::Fixed(300.0)),
            )
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::color!(0x31, 0x32, 0x44))),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: iced::color!(0x45, 0x47, 0x5a),
                },
                ..Default::default()
            });

            let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

            return stack![base_layout, backdrop, centered_popup,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        if self.show_shell_picker {
            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.4,
                        })),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CloseShellPicker);

            let popup_card = container({
                let mut items: Vec<Element<Message>> = Vec::new();
                let mut option_index = 0usize;

                #[cfg(target_family = "unix")]
                {
                    items.push(
                        if self.shell_picker_selected == option_index {
                            button_primary("Shell")
                        } else {
                            button_secondary("Shell")
                        }
                        .on_press(Message::CreateTab(ShellKind::Zsh))
                        .width(Length::Fill)
                        .into(),
                    );
                    option_index += 1;
                }

                #[cfg(target_family = "windows")]
                {
                    items.push(
                        if self.shell_picker_selected == option_index {
                            button_primary("cmd")
                        } else {
                            button_secondary("cmd")
                        }
                        .on_press(Message::CreateTab(ShellKind::Cmd))
                        .width(Length::Fill)
                        .into(),
                    );
                    option_index += 1;

                    items.push(
                        if self.shell_picker_selected == option_index {
                            button_primary("PowerShell")
                        } else {
                            button_secondary("PowerShell")
                        }
                        .on_press(Message::CreateTab(ShellKind::PowerShell))
                        .width(Length::Fill)
                        .into(),
                    );
                    option_index += 1;
                }

                for (i, profile) in self.config.ssh_profiles.iter().enumerate() {
                    let label: &str = if profile.name.is_empty() {
                        &profile.host
                    } else {
                        &profile.name
                    };
                    let selected = self.shell_picker_selected == option_index;
                    items.push(ssh_picker_button(label, selected, Message::CreateSshTab(i)));
                    option_index += 1;
                }

                items.push(
                    if self.shell_picker_selected == self.shell_picker_option_count() - 1 {
                        button_primary("Cancel")
                    } else {
                        button_secondary("Cancel")
                    }
                    .on_press(Message::CloseShellPicker)
                    .width(Length::Fill)
                    .into(),
                );

                column(items)
                    .spacing(10)
                    .padding(20)
                    .width(Length::Fixed(220.0))
            })
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::color!(0x31, 0x32, 0x44))),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: iced::color!(0x45, 0x47, 0x5a),
                },
                ..Default::default()
            });

            let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

            stack![base_layout, backdrop, centered_popup,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            base_layout.into()
        }
    }

    fn view_config(&self) -> Element<'_, Message> {
        let palette = Palette::DARK;
        let mut category_items: Vec<Element<Message>> = Vec::new();

        for category in SettingsCategory::ALL {
            let is_active = category == self.settings_category;
            let label = category.label();
            let button_style = move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                let base_bg = if is_active {
                    Color {
                        a: 0.35,
                        ..palette.background
                    }
                } else {
                    Color::TRANSPARENT
                };
                let hover_bg = if is_active {
                    base_bg
                } else {
                    Color {
                        a: 0.2,
                        ..palette.background
                    }
                };

                let background = match status {
                    iced::widget::button::Status::Hovered => hover_bg,
                    _ => base_bg,
                };

                iced::widget::button::Style {
                    background: Some(Background::Color(background)),
                    text_color: if is_active {
                        palette.text
                    } else {
                        palette.text_secondary
                    },
                    border: Border {
                        radius: RADIUS_NORMAL.into(),
                        width: if is_active { 1.0 } else { 0.0 },
                        color: Color {
                            a: 0.15,
                            ..palette.text
                        },
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            };

            let item = button(text(label).size(13))
                .padding([6, 10])
                .width(Length::Fill)
                .on_press(Message::SelectSettingsCategory(category))
                .style(button_style);
            category_items.push(item.into());
        }

        let sidebar = container(
            column(category_items)
                .spacing(SPACING_SMALL)
                .padding(SPACING_NORMAL)
                .width(Length::Fill),
        )
        .width(Length::Fixed(180.0))
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            ..Default::default()
        });

        let breadcrumb = row![
            text("Settings").size(18),
            text("/").size(16).color(Color {
                a: 0.3,
                ..palette.text
            }),
            text(self.settings_category.label())
                .size(16)
                .color(palette.accent),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SPACING_SMALL);

        let header = row![
            breadcrumb,
            container("").width(Length::Fill),
            row![
                button_secondary("Apply").on_press(Message::ApplySettings),
                button_primary("Save").on_press(Message::SaveSettings),
            ]
            .spacing(SPACING_SMALL)
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill);

        let body_content = container(settings::view_category(
            self.settings_category,
            &self.config,
            &self.settings_draft,
            &self.terminal_font_options,
        ))
        .padding([0, 12])
        .width(Length::Fill);

        let body = scrollable(body_content)
            .height(Length::Fill)
            .width(Length::Fill);

        let content = container(
            column(vec![header.into(), body.into()])
                .spacing(SPACING_NORMAL)
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SPACING_LARGE);

        row![sidebar, content]
            .spacing(SPACING_LARGE)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }
}

fn ssh_picker_button(label: &str, selected: bool, on_press: Message) -> Element<'_, Message> {
    let palette = Palette::DARK;
    let prefix = format!("SSH: {label}");
    if selected {
        button(text(prefix))
            .style(
                move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                    let base = iced::widget::button::Style {
                        background: Some(Background::Color(palette.accent)),
                        text_color: palette.background,
                        border: Border {
                            radius: RADIUS_NORMAL.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    };
                    match status {
                        iced::widget::button::Status::Hovered => iced::widget::button::Style {
                            background: Some(Background::Color(Color {
                                a: 0.9,
                                ..palette.accent
                            })),
                            ..base
                        },
                        _ => base,
                    }
                },
            )
            .on_press(on_press)
            .width(Length::Fill)
            .into()
    } else {
        button(text(prefix))
            .style(
                move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                    let base = iced::widget::button::Style {
                        background: Some(Background::Color(palette.surface)),
                        text_color: palette.text,
                        border: Border {
                            radius: RADIUS_NORMAL.into(),
                            width: 1.0,
                            color: Color {
                                a: 0.1,
                                ..palette.text
                            },
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    };
                    match status {
                        iced::widget::button::Status::Hovered => iced::widget::button::Style {
                            background: Some(Background::Color(Color {
                                a: 0.8,
                                ..palette.surface
                            })),
                            border: Border {
                                color: Color {
                                    a: 0.3,
                                    ..palette.text
                                },
                                ..base.border
                            },
                            ..base
                        },
                        _ => base,
                    }
                },
            )
            .on_press(on_press)
            .width(Length::Fill)
            .into()
    }
}
