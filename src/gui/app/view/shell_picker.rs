use super::super::{App, Message};
use crate::gui::icons::{self, ShellIcon};
use crate::gui::tab::{Profile, ProfileKind};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_SMALL};
use iced::time::Instant;
use iced::widget::{button, column, container, mouse_area, row, scrollable, stack, svg, text};
use iced::{Background, Border, Color, Element, Length};

#[derive(Clone, Copy)]
struct PickerStyle {
    alpha: f32,
    palette: Palette,
    animations_enabled: bool,
}

impl PickerStyle {
    fn text(&self, label: impl ToString, size: f32) -> Element<'static, Message> {
        text(label.to_string())
            .size(size)
            .color(Color {
                a: self.alpha,
                ..self.palette.text
            })
            .into()
    }

    fn text_secondary(&self, label: impl ToString, size: f32) -> Element<'static, Message> {
        text(label.to_string())
            .size(size)
            .color(Color {
                a: self.alpha * 0.7,
                ..self.palette.text_secondary
            })
            .into()
    }

    fn icon(&self, shell_icon: ShellIcon) -> Element<'static, Message> {
        let alpha = self.alpha;
        let icon_color = shell_icon.color;
        svg(shell_icon.handle)
            .width(Length::Fixed(16.0))
            .height(Length::Fixed(16.0))
            .opacity(alpha)
            .style(move |_theme: &iced::Theme, _status| svg::Style {
                color: Some(icon_color),
            })
            .into()
    }

    fn emoji(&self, glyph: String) -> Element<'static, Message> {
        container(text(glyph).size(14.0).color(Color {
            a: self.alpha,
            ..self.palette.text
        }))
        .width(Length::Fixed(16.0))
        .align_x(iced::Alignment::Center)
        .into()
    }

    fn divider(&self) -> Element<'static, Message> {
        let alpha = self.alpha;
        let palette = self.palette;
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

    fn item_button(
        &self,
        icon: Element<'static, Message>,
        label: String,
        subtitle: Option<String>,
        selected: bool,
        on_press: Message,
    ) -> Element<'static, Message> {
        let alpha = self.alpha;
        let palette = self.palette;

        let mut label_items: Vec<Element<'static, Message>> = vec![self.text(&label, 13.0)];
        if let Some(sub) = subtitle {
            label_items.push(self.text_secondary(&sub, 10.0));
        }

        let content = row![icon, column(label_items).spacing(1)]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        // Background painted by `hover_fade` behind the button so the hover
        // transition matches the rest of the app.
        let inner = button(content)
            .style(
                move |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                    iced::widget::button::Style {
                        background: Some(Background::Color(Color::TRANSPARENT)),
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
                },
            )
            .padding([6, 10])
            .on_press(on_press)
            .width(Length::Fill);

        let rest = crate::gui::components::HoverStyle {
            background: if selected {
                Color {
                    a: 0.15 * alpha,
                    ..palette.text
                }
            } else {
                Color::TRANSPARENT
            },
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: RADIUS_SMALL,
        };
        let hover = if selected {
            rest
        } else {
            crate::gui::components::HoverStyle {
                background: Color {
                    a: 0.08 * alpha,
                    ..palette.text
                },
                ..rest
            }
        };

        crate::gui::components::hover_fade(inner, rest, hover, self.animations_enabled).into()
    }
}

fn icon_for_shell(shell: &Profile) -> ShellIcon {
    if let Some(name) = shell.icon.as_deref().filter(|n| !n.trim().is_empty()) {
        return icons::by_name(name);
    }
    match &shell.kind {
        ProfileKind::Ssh(_) => icons::ssh(),
        ProfileKind::Local { program: None, .. } => icons::by_name(&icons::default_shell_name()),
        ProfileKind::Local { .. } => icons::by_name(&shell.name),
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

        let progress: f32 = self.modal_anim.interpolate(0.0f32, 1.0f32, now);

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

        let style = PickerStyle {
            alpha: progress,
            palette,
            animations_enabled: self.config.ui.animations_enabled,
        };

        let mut items: Vec<Element<Message>> = Vec::new();
        let mut option_index = 0usize;

        items.push(style.text(t!("shell_picker.title"), 15.0));
        items.push(style.divider());

        let ssh_profiles = self.session_ssh_profiles();
        if !ssh_profiles.is_empty() {
            items.push(style.text_secondary(t!("shell_picker.ssh"), 11.0));

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
                let icon = style.icon(icons::ssh());
                items.push(style.item_button(
                    icon,
                    label,
                    subtitle,
                    selected,
                    Message::CreateSshTab(i),
                ));
                option_index += 1;
            }
            items.push(style.divider());
        }

        let local_profiles = self.session_local_profiles();
        if !local_profiles.is_empty() {
            items.push(style.text_secondary(t!("shell_picker.profiles"), 11.0));

            for profile in &local_profiles {
                let subtitle = match &profile.kind {
                    ProfileKind::Local {
                        program: Some(path),
                        ..
                    } => Some(path.clone()),
                    _ => Some(t!("settings.ssh.default_shell").into()),
                };
                let selected = self.shell_picker_selected == option_index;
                let icon = match &profile.icon {
                    Some(glyph) if !glyph.trim().is_empty() => style.emoji(glyph.clone()),
                    _ => style.icon(icon_for_shell(profile)),
                };
                items.push(style.item_button(
                    icon,
                    profile.display_name(),
                    subtitle,
                    selected,
                    Message::CreateTab(profile.clone()),
                ));
                option_index += 1;
            }
            items.push(style.divider());
        }

        items.push(style.text_secondary(t!("shell_picker.shells"), 11.0));

        for shell in &self.available_shells {
            let label = shell.display_name();
            let subtitle = match &shell.kind {
                ProfileKind::Local { program: None, .. } => Some(t!("shell_picker.default").into()),
                ProfileKind::Local {
                    program: Some(path),
                    ..
                } => Some(path.clone()),
                ProfileKind::Ssh(_) => None,
            };
            let selected = self.shell_picker_selected == option_index;
            let icon = style.icon(icon_for_shell(shell));
            items.push(style.item_button(
                icon,
                label,
                subtitle,
                selected,
                Message::CreateTab(shell.clone()),
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
