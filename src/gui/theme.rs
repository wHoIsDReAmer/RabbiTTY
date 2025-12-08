#![allow(unused)]

use iced::{Color, color};

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
        accent: color!(0x89, 0xb4, 0xfa),     // Blue
        success: color!(0xa6, 0xe3, 0xa1),    // Green
        error: color!(0xf3, 0x8b, 0xa8),      // Red
    };
}

pub const SPACING_SMALL: u16 = 4;
pub const SPACING_NORMAL: u16 = 8;
pub const SPACING_LARGE: u16 = 16;

pub const RADIUS_SMALL: f32 = 4.0;
pub const RADIUS_NORMAL: f32 = 8.0;
