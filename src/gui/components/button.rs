use crate::gui::theme::{Palette, RADIUS_NORMAL};
use iced::widget::button;
use iced::{Background, Border, Color, Theme};

pub fn primary(text: &str) -> button::Button<'_, crate::gui::app::Message> {
    button(text).style(|_theme: &Theme, status: button::Status| {
        let palette = Palette::DARK;
        let base = button::Style {
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
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.9,
                    ..palette.accent
                })),
                ..base
            },
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.8,
                    ..palette.accent
                })),
                ..base
            },
            button::Status::Disabled => button::Style {
                background: Some(Background::Color(palette.surface)),
                text_color: palette.text_secondary,
                ..base
            },
        }
    })
}

pub fn secondary(text: &str) -> button::Button<'_, crate::gui::app::Message> {
    button(text).style(|_theme: &Theme, status: button::Status| {
        let palette = Palette::DARK;
        let base = button::Style {
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
            button::Status::Active => base,
            button::Status::Hovered => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.8, // Slightly lighter/transparent
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
            button::Status::Pressed => button::Style {
                background: Some(Background::Color(Color {
                    a: 0.6,
                    ..palette.surface
                })),
                ..base
            },
            button::Status::Disabled => button::Style {
                text_color: palette.text_secondary,
                background: Some(Background::Color(Color {
                    a: 0.5,
                    ..palette.surface
                })),
                ..base
            },
        }
    })
}
