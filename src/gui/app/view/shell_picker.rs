use super::super::{App, Message};
use crate::gui::tab::ShellKind;
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_SMALL};
use iced::time::Instant;
use iced::widget::{button, column, container, mouse_area, row, scrollable, stack, svg, text};
use iced::{Background, Border, Color, Element, Length};
use std::sync::LazyLock;

static ICON_BASH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../../assets/icons/bash.svg")));
static ICON_ZSH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../../assets/icons/zsh.svg")));
static ICON_FISH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../../assets/icons/fish.svg")));
static ICON_POWERSHELL: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../../assets/icons/powershell.svg"))
});
static ICON_TERMINAL: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../../assets/icons/terminal.svg"))
});
static ICON_SSH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../../assets/icons/ssh.svg")));

struct ShellIcon {
    handle: svg::Handle,
    color: Color,
}

fn icon_by_name(name: &str) -> ShellIcon {
    match name.to_lowercase().as_str() {
        "bash" => ShellIcon {
            handle: ICON_BASH.clone(),
            color: Color::from_rgb8(0x4E, 0xAA, 0x25),
        },
        "zsh" => ShellIcon {
            handle: ICON_ZSH.clone(),
            color: Color::from_rgb8(0xF1, 0x5A, 0x24),
        },
        "fish" => ShellIcon {
            handle: ICON_FISH.clone(),
            color: Color::from_rgb8(0x34, 0xC5, 0x34),
        },
        "pwsh" | "powershell" => ShellIcon {
            handle: ICON_POWERSHELL.clone(),
            color: Color::from_rgb8(0x5A, 0x91, 0xD8),
        },
        _ => ShellIcon {
            handle: ICON_TERMINAL.clone(),
            color: Color::from_rgb8(0x4C, 0xC2, 0xFF),
        },
    }
}

fn icon_for_shell(shell: &ShellKind) -> ShellIcon {
    match shell {
        ShellKind::Ssh(_) => ShellIcon {
            handle: ICON_SSH.clone(),
            color: Color::from_rgb8(0x4F, 0xC0, 0x8D),
        },
        ShellKind::Default => {
            let name = std::env::var("SHELL").unwrap_or_default();
            let name = std::path::Path::new(&name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            icon_by_name(&name)
        }
        ShellKind::Shell { name, .. } => icon_by_name(name),
    }
}

const PICKER_WIDTH: f32 = 280.0;

impl App {
    pub(in crate::gui) fn view_shell_picker<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let palette = self.palette;
        let now = Instant::now();

        let progress: f32 = self.shell_picker_anim.interpolate(0.0f32, 1.0f32, now);

        let backdrop_alpha = 0.5 * progress;

        let backdrop = mouse_area(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: backdrop_alpha,
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
                .color(Color {
                    a: progress,
                    ..palette.text
                })
                .into(),
        );
        items.push(divider_with_alpha(progress, palette));

        // SSH section first: profiles should be visible without scrolling past system shells.
        let ssh_profiles = self.session_ssh_profiles();
        if !ssh_profiles.is_empty() {
            items.push(section_label("SSH", progress, palette));

            for (i, profile) in ssh_profiles.iter().enumerate() {
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
                    progress,
                    palette,
                    ShellIcon {
                        handle: (*ICON_SSH).clone(),
                        color: Color::from_rgb8(0x4F, 0xC0, 0x8D),
                    },
                ));
                option_index += 1;
            }
            items.push(divider_with_alpha(progress, palette));
        }

        // Shell section
        items.push(section_label("Shells", progress, palette));

        for shell in &self.available_shells {
            let label = shell.display_name();
            let subtitle = match shell {
                ShellKind::Default => Some("Default".into()),
                ShellKind::Shell { path, .. } => Some(path.clone()),
                ShellKind::Ssh(_) => None,
            };
            let selected = self.shell_picker_selected == option_index;
            let icon = icon_for_shell(shell);
            items.push(picker_item(
                label,
                subtitle,
                selected,
                Message::CreateTab(shell.clone()),
                progress,
                palette,
                icon,
            ));
            option_index += 1;
        }

        let card_alpha = 0.98 * progress;
        let border_alpha = 0.15 * progress;

        let picker_list = scrollable(
            column(items)
                .spacing(SPACING_SMALL)
                .padding([16, 12])
                .width(Length::Fixed(PICKER_WIDTH)),
        )
        .height(Length::Fixed(460.0))
        .width(Length::Fixed(PICKER_WIDTH))
        .style(move |_theme: &iced::Theme, status: scrollable::Status| {
            use iced::widget::container;
            use iced::widget::scrollable::{AutoScroll, Rail, Scroller, Style};
            use iced::{Background, Border, Shadow};

            let scroller_alpha = match status {
                scrollable::Status::Active { .. } => 0.45,
                scrollable::Status::Hovered { .. } => 0.65,
                scrollable::Status::Dragged { .. } => 0.8,
            } * progress;

            let rail = |visible: bool| Rail {
                background: Some(Background::Color(if visible {
                    Color {
                        a: 0.08 * progress,
                        ..palette.surface
                    }
                } else {
                    Color::TRANSPARENT
                })),
                border: Border::default(),
                scroller: Scroller {
                    background: Background::Color(Color {
                        a: if visible { scroller_alpha } else { 0.0 },
                        ..palette.text_secondary
                    }),
                    border: Border {
                        radius: RADIUS_SMALL.into(),
                        ..Default::default()
                    },
                },
            };

            Style {
                container: container::Style::default(),
                vertical_rail: rail(true),
                horizontal_rail: rail(false),
                gap: None,
                auto_scroll: AutoScroll {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    shadow: Shadow::default(),
                    icon: Color::TRANSPARENT,
                },
            }
        });

        let popup_card =
            container(picker_list).style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: palette.surface.r * 0.9,
                    g: palette.surface.g * 0.9,
                    b: palette.surface.b * 0.9,
                    a: card_alpha,
                })),
                border: Border {
                    radius: (RADIUS_NORMAL + 4.0).into(),
                    width: 1.0,
                    color: Color {
                        a: border_alpha,
                        ..palette.text
                    },
                },
                ..Default::default()
            });

        // Slide up: start 16px below center, ease to 0
        let slide_offset = 16.0 * (1.0 - progress);
        let centered_popup: Element<Message> = container(popup_card)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .padding(iced::Padding::new(0.0).top(slide_offset))
            .into();

        stack![base_layout.into(), backdrop, centered_popup]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

fn section_label(label: &str, alpha: f32, palette: Palette) -> Element<'_, Message> {
    text(label)
        .size(11)
        .color(Color {
            a: alpha * 0.7,
            ..palette.text_secondary
        })
        .into()
}

fn divider_with_alpha(alpha: f32, palette: Palette) -> Element<'static, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(1)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.1 * alpha,
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
    alpha: f32,
    palette: Palette,
    shell_icon: ShellIcon,
) -> Element<'static, Message> {
    let icon_color = shell_icon.color;
    let icon = svg(shell_icon.handle)
        .width(Length::Fixed(16.0))
        .height(Length::Fixed(16.0))
        .style(move |_theme: &iced::Theme, _status| svg::Style {
            color: Some(Color {
                a: alpha,
                ..icon_color
            }),
        });

    let mut label_items: Vec<Element<'static, Message>> = vec![
        text(label)
            .size(13)
            .color(Color {
                a: alpha,
                ..palette.text
            })
            .into(),
    ];

    if let Some(sub) = subtitle {
        label_items.push(
            text(sub)
                .size(10)
                .color(Color {
                    a: alpha * 0.7,
                    ..palette.text_secondary
                })
                .into(),
        );
    }

    let content = row![icon, column(label_items).spacing(1)]
        .spacing(10)
        .align_y(iced::Alignment::Center);

    button(content)
        .style(
            move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                let hovered = matches!(status, iced::widget::button::Status::Hovered);
                if selected {
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color {
                            a: 0.15 * alpha,
                            ..palette.text
                        })),
                        text_color: Color {
                            a: alpha,
                            ..palette.text
                        },
                        border: Border {
                            radius: RADIUS_SMALL.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }
                } else {
                    iced::widget::button::Style {
                        background: Some(Background::Color(if hovered {
                            Color {
                                a: 0.08 * alpha,
                                ..palette.text
                            }
                        } else {
                            Color::TRANSPARENT
                        })),
                        text_color: Color {
                            a: alpha,
                            ..palette.text
                        },
                        border: Border {
                            radius: RADIUS_SMALL.into(),
                            width: 0.0,
                            color: Color::TRANSPARENT,
                        },
                        shadow: iced::Shadow::default(),
                        snap: false,
                    }
                }
            },
        )
        .padding([6, 10])
        .on_press(on_press)
        .width(Length::Fill)
        .into()
}
