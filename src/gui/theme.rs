#![allow(unused)]

use iced::widget::{container, scrollable};
use iced::{Background, Border, Color, Shadow, color};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub background: Color,
    pub surface: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub accent: Color,
    pub success: Color,
    pub error: Color,
}

impl Palette {
    pub const DARK: Self = Self {
        background: color!(0x1e, 0x1e, 0x2e), // Catppuccin Mocha Base
        surface: color!(0x31, 0x32, 0x44),    // Surface0
        text: color!(0xcd, 0xd6, 0xf4),       // Text
        text_secondary: color!(0xa6, 0xad, 0xc8), // Subtext0
        accent: color!(0x4d, 0x9e, 0xf7),     // Bluebird
        success: color!(0xa6, 0xe3, 0xa1),    // Green
        error: color!(0xf3, 0x8b, 0xa8),      // Red
    };

    pub fn from_theme(theme: &crate::config::ThemeConfig) -> Self {
        let bg = theme.background;
        let fg = theme.foreground;

        // Elevated surfaces move away from the background: lighter on dark
        // themes, darker on light ones.
        let dark = u16::from(bg[0]) + u16::from(bg[1]) + u16::from(bg[2]) < 384;
        let lift = |c: u8, d: u8| {
            if dark {
                c.saturating_add(d)
            } else {
                c.saturating_sub(d)
            }
        };
        let surface = [lift(bg[0], 26), lift(bg[1], 27), lift(bg[2], 30)];
        let text_sec = [
            blend_u8(fg[0], bg[0], 0.35),
            blend_u8(fg[1], bg[1], 0.35),
            blend_u8(fg[2], bg[2], 0.35),
        ];

        Self {
            background: Color::from_rgb8(bg[0], bg[1], bg[2]),
            surface: Color::from_rgb8(surface[0], surface[1], surface[2]),
            text: Color::from_rgb8(fg[0], fg[1], fg[2]),
            text_secondary: Color::from_rgb8(text_sec[0], text_sec[1], text_sec[2]),
            accent: color!(0x4d, 0x9e, 0xf7),
            success: color!(0xa6, 0xe3, 0xa1),
            error: color!(0xf3, 0x8b, 0xa8),
        }
    }
}

fn blend_u8(from: u8, to: u8, t: f32) -> u8 {
    (from as f32 + (to as f32 - from as f32) * t) as u8
}

pub const SPACING_SMALL: f32 = 4.0;
pub const SPACING_NORMAL: f32 = 8.0;
pub const SPACING_LARGE: f32 = 16.0;

pub const RADIUS_SMALL: f32 = 4.0;
pub const RADIUS_NORMAL: f32 = 8.0;

pub fn scrollbar_style(
    palette: Palette,
) -> impl Fn(&iced::Theme, scrollable::Status) -> scrollable::Style {
    move |_theme: &iced::Theme, status: scrollable::Status| {
        let scroller_alpha = match status {
            scrollable::Status::Active { .. } => 0.45,
            scrollable::Status::Hovered { .. } => 0.65,
            scrollable::Status::Dragged { .. } => 0.8,
        };

        let rail = |visible: bool| scrollable::Rail {
            background: Some(Background::Color(if visible {
                Color {
                    a: 0.08,
                    ..palette.surface
                }
            } else {
                Color::TRANSPARENT
            })),
            border: Border::default(),
            scroller: scrollable::Scroller {
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

        scrollable::Style {
            container: container::Style::default(),
            vertical_rail: rail(true),
            horizontal_rail: rail(true),
            gap: None,
            auto_scroll: scrollable::AutoScroll {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                shadow: Shadow::default(),
                icon: Color::TRANSPARENT,
            },
        }
    }
}
