use super::super::{App, Message};
use crate::gui::components::{button_primary, button_secondary};
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{button, center, column, container, mouse_area, row, stack, text};
use iced::{Background, Border, Color, Element, Length};

impl App {
    pub(in crate::gui) fn view_shell_picker<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
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

            for shell in &self.available_shells {
                let label = shell.display_name();
                let selected = self.shell_picker_selected == option_index;
                items.push(shell_option_button(
                    label,
                    selected,
                    Message::CreateTab(shell.clone()),
                ));
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

        stack![base_layout.into(), backdrop, centered_popup,]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    #[cfg(target_os = "macos")]
    pub(in crate::gui) fn view_restart_confirm<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
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

        stack![base_layout.into(), backdrop, centered_popup,]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn shell_option_button(
    label: String,
    selected: bool,
    on_press: Message,
) -> Element<'static, Message> {
    let palette = Palette::DARK;
    if selected {
        button(text(label))
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
        button(text(label))
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
