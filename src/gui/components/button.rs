//! Button factories with built-in hover-fade animation.
//!
//! Every button rendered by the app should ultimately go through one of these
//! factories so the hover transition stays visually consistent across screens.
//! The button itself stays transparent; the animated background + border is
//! painted behind it by [`hover_fade`].

use crate::gui::app::Message;
use crate::gui::components::{HoverStyle, hover_fade};
use crate::gui::theme::Palette;
use iced::widget::{button, text};
use iced::{Background, Border, Color, Element, Length, Shadow, Theme};

const RADIUS: f32 = 6.0;
const ICON_RADIUS: f32 = 6.0;
const MENU_RADIUS: f32 = 4.0;

/// Accent-filled primary button (e.g. Save, confirm).
pub fn primary<'a>(
    label: impl AsRef<str>,
    on_press: Message,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let inner = button(text(label.as_ref().to_string()).size(13))
        .padding([7, 16])
        .on_press(on_press)
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: match status {
                    button::Status::Disabled => palette.text_secondary,
                    _ => palette.background,
                },
                border: Border {
                    radius: RADIUS.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                snap: true,
            },
        );

    let rest = HoverStyle {
        background: palette.accent,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: RADIUS,
    };
    let hover = HoverStyle {
        background: Color {
            r: (palette.accent.r * 1.1).clamp(0.0, 1.0),
            g: (palette.accent.g * 1.1).clamp(0.0, 1.0),
            b: (palette.accent.b * 1.1).clamp(0.0, 1.0),
            a: 1.0,
        },
        ..rest
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}

/// Surface-tinted secondary button (e.g. Cancel). If `on_press` is `None`
/// the button is rendered in a disabled state.
pub fn secondary<'a>(
    label: impl AsRef<str>,
    on_press: Option<Message>,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let enabled = on_press.is_some();
    let mut inner = button(text(label.as_ref().to_string()).size(13))
        .padding([7, 16])
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: if enabled {
                    match status {
                        button::Status::Disabled => palette.text_secondary,
                        _ => palette.text,
                    }
                } else {
                    palette.text_secondary
                },
                border: Border {
                    radius: RADIUS.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                snap: true,
            },
        );
    if let Some(msg) = on_press {
        inner = inner.on_press(msg);
    }

    let rest = HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color {
            a: 0.1,
            ..palette.text
        },
        border_width: 1.0,
        radius: RADIUS,
    };
    let hover = if enabled {
        HoverStyle {
            background: Color {
                a: 0.12,
                ..palette.text
            },
            border_color: Color {
                a: 0.2,
                ..palette.text
            },
            border_width: 1.0,
            radius: RADIUS,
        }
    } else {
        rest
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}

/// Small glyph/icon button used for tab-bar controls, SSH row actions, etc.
pub fn icon<'a>(
    glyph: impl AsRef<str>,
    on_press: Message,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let inner = button(text(glyph.as_ref().to_string()).size(13))
        .padding([6, 10])
        .on_press(on_press)
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: match status {
                    button::Status::Hovered => palette.text,
                    _ => palette.text_secondary,
                },
                border: Border {
                    radius: ICON_RADIUS.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                snap: true,
            },
        );

    let rest = HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: ICON_RADIUS,
    };
    let hover = HoverStyle {
        background: Color {
            a: 0.1,
            ..palette.text
        },
        ..rest
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}

/// Same as [`icon`] but with an active/toggled state that keeps an accent
/// background even when the cursor is not over the button (used by the SFTP
/// toggle on the tab bar).
pub fn icon_toggle<'a>(
    glyph: impl AsRef<str>,
    on_press: Message,
    active: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    icon_toggle_content(
        text(glyph.as_ref().to_string()).size(13).into(),
        on_press,
        active,
        palette,
        animations_enabled,
    )
}

pub fn icon_toggle_content<'a>(
    content: Element<'a, Message>,
    on_press: Message,
    active: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let inner = button(content).padding([6, 10]).on_press(on_press).style(
        move |_theme: &Theme, status: button::Status| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: if active {
                palette.accent
            } else {
                match status {
                    button::Status::Hovered => palette.text,
                    _ => palette.text_secondary,
                }
            },
            border: Border {
                radius: ICON_RADIUS.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        },
    );

    let rest = HoverStyle {
        background: if active {
            Color {
                a: 0.18,
                ..palette.accent
            }
        } else {
            Color::TRANSPARENT
        },
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: ICON_RADIUS,
    };
    let hover = HoverStyle {
        background: Color {
            a: 0.12,
            ..palette.text
        },
        ..rest
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}

/// Full-width context-menu row.
pub fn menu_item<'a>(
    label: impl AsRef<str>,
    on_press: Message,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let inner = button(text(label.as_ref().to_string()).size(13))
        .padding([7, 14])
        .width(Length::Fill)
        .on_press(on_press)
        .style(
            move |_theme: &Theme, _status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: palette.text,
                border: Border {
                    radius: MENU_RADIUS.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                snap: false,
            },
        );

    let rest = HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: MENU_RADIUS,
    };
    let hover = HoverStyle {
        background: Color {
            a: 0.1,
            ..palette.text
        },
        ..rest
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}
