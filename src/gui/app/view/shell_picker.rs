use super::super::{App, Message};
use crate::gui::tab::ShellKind;
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_SMALL};
use iced::widget::{button, center, column, container, mouse_area, stack, text};
use iced::{Background, Border, Color, Element, Length};

const PICKER_WIDTH: f32 = 280.0;

impl App {
    pub(in crate::gui) fn view_shell_picker<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let palette = Palette::DARK;

        let backdrop = mouse_area(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.4,
                    })),
                    ..Default::default()
                }),
        )
        .on_press(Message::CloseShellPicker);

        let mut items: Vec<Element<Message>> = Vec::new();
        let mut option_index = 0usize;

        // Header
        items.push(
            text("Start New Session")
                .size(15)
                .color(palette.text)
                .into(),
        );
        items.push(divider());

        // Shell section
        items.push(section_label("Shells"));

        for shell in &self.available_shells {
            let label = shell.display_name();
            let subtitle = match shell {
                ShellKind::Default => Some("Default".into()),
                ShellKind::Shell { path, .. } => Some(path.clone()),
                ShellKind::Ssh(_) => None,
            };
            let selected = self.shell_picker_selected == option_index;
            items.push(picker_item(
                label,
                subtitle,
                selected,
                Message::CreateTab(shell.clone()),
            ));
            option_index += 1;
        }

        // SSH section
        if !self.config.ssh_profiles.is_empty() {
            items.push(divider());
            items.push(section_label("SSH"));

            for (i, profile) in self.config.ssh_profiles.iter().enumerate() {
                let label = if profile.name.is_empty() {
                    profile.host.clone()
                } else {
                    profile.name.clone()
                };
                let subtitle = if profile.user.is_empty() {
                    Some(format!("{}:{}", profile.host, profile.port))
                } else {
                    Some(format!(
                        "{}@{}:{}",
                        profile.user, profile.host, profile.port
                    ))
                };
                let selected = self.shell_picker_selected == option_index;
                items.push(picker_item(
                    label,
                    subtitle,
                    selected,
                    Message::CreateSshTab(i),
                ));
                option_index += 1;
            }
        }

        let popup_card = container(
            column(items)
                .spacing(SPACING_SMALL)
                .padding([16, 12])
                .width(Length::Fixed(PICKER_WIDTH)),
        )
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                r: palette.surface.r * 0.9,
                g: palette.surface.g * 0.9,
                b: palette.surface.b * 0.9,
                a: 0.98,
            })),
            border: Border {
                radius: (RADIUS_NORMAL + 4.0).into(),
                width: 1.0,
                color: Color {
                    a: 0.15,
                    ..palette.text
                },
            },
            ..Default::default()
        });

        let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

        stack![base_layout.into(), backdrop, centered_popup]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn section_label(label: &str) -> Element<'_, Message> {
    let palette = Palette::DARK;
    text(label).size(11).color(palette.text_secondary).into()
}

fn divider() -> Element<'static, Message> {
    let palette = Palette::DARK;
    container(text(""))
        .width(Length::Fill)
        .height(1)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.1,
                ..palette.text
            })),
            ..Default::default()
        })
        .into()
}

fn picker_item(
    label: String,
    subtitle: Option<String>,
    selected: bool,
    on_press: Message,
) -> Element<'static, Message> {
    let palette = Palette::DARK;

    let mut content_items: Vec<Element<'static, Message>> = vec![
        text(label)
            .size(13)
            .color(if selected {
                palette.background
            } else {
                palette.text
            })
            .into(),
    ];

    if let Some(sub) = subtitle {
        content_items.push(
            text(sub)
                .size(10)
                .color(if selected {
                    Color {
                        a: 0.7,
                        ..palette.background
                    }
                } else {
                    palette.text_secondary
                })
                .into(),
        );
    }

    let content = column(content_items).spacing(1);

    button(content)
        .style(
            move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                if selected {
                    iced::widget::button::Style {
                        background: Some(Background::Color(palette.accent)),
                        text_color: palette.background,
                        border: Border {
                            radius: RADIUS_SMALL.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    }
                } else {
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
                        snap: true,
                    }
                }
            },
        )
        .padding([6, 10])
        .on_press(on_press)
        .width(Length::Fill)
        .into()
}
