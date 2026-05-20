//! Accent-driven style helpers for `toggler` and `combo_box` widgets so they
//! visually match the rest of the UI (which derives colors from
//! `palette.accent`) instead of iced's default blue accent.

use crate::gui::theme::{Palette, RADIUS_SMALL};
use iced::widget::overlay::menu;
use iced::widget::{text_input, toggler};
use iced::{Background, Border, Color, Shadow, Theme};

/// Style closure for `toggler` widgets driven by `palette.accent`.
///
/// Visual treatment:
/// - On + active/hovered → accent background, knob = `palette.background`.
/// - Off + active        → dim text-tinted background, knob = secondary text.
/// - Hovered             → slightly brighter than the active variant.
/// - Disabled            → faded to ~30% alpha to read as inert.
pub fn accent_toggler_style(
    palette: Palette,
) -> impl Fn(&Theme, toggler::Status) -> toggler::Style {
    move |_theme: &Theme, status: toggler::Status| {
        // Off-state background: a subtle text-tinted fill matching the
        // visual weight of the `button_secondary` resting state.
        let off_bg = Color {
            a: 0.18,
            ..palette.text
        };
        let off_bg_hover = Color {
            a: 0.26,
            ..palette.text
        };

        // On-state background: the palette accent, slightly brightened on
        // hover (mirrors `button_primary`).
        let on_bg = palette.accent;
        let on_bg_hover = Color {
            r: (palette.accent.r * 1.1).min(1.0),
            g: (palette.accent.g * 1.1).min(1.0),
            b: (palette.accent.b * 1.1).min(1.0),
            a: 1.0,
        };

        let (background, foreground) = match status {
            toggler::Status::Active { is_toggled: true } => (on_bg, palette.background),
            toggler::Status::Active { is_toggled: false } => (off_bg, palette.text_secondary),
            toggler::Status::Hovered { is_toggled: true } => (on_bg_hover, palette.background),
            toggler::Status::Hovered { is_toggled: false } => (off_bg_hover, palette.text),
            toggler::Status::Disabled { is_toggled: true } => (
                Color {
                    a: 0.3,
                    ..palette.accent
                },
                Color {
                    a: 0.5,
                    ..palette.background
                },
            ),
            toggler::Status::Disabled { is_toggled: false } => (
                Color {
                    a: 0.08,
                    ..palette.text
                },
                Color {
                    a: 0.3,
                    ..palette.text_secondary
                },
            ),
        };

        toggler::Style {
            background: Background::Color(background),
            background_border_width: 0.0,
            background_border_color: Color::TRANSPARENT,
            foreground: Background::Color(foreground),
            foreground_border_width: 0.0,
            foreground_border_color: Color::TRANSPARENT,
            text_color: None,
            border_radius: None,
            padding_ratio: 0.1,
        }
    }
}

/// Style closure for the text-input portion of a `combo_box`.
/// Mirrors the project's `styled_text_input`: subtle background, accent
/// border on focus, accent-tinted selection.
pub fn accent_combo_box_input_style(
    palette: Palette,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_theme: &Theme, status: text_input::Status| {
        let focused = matches!(status, text_input::Status::Focused { .. });
        let hovered = matches!(status, text_input::Status::Hovered);

        let border_color = if focused {
            Color {
                a: 0.5,
                ..palette.accent
            }
        } else if hovered {
            Color {
                a: 0.25,
                ..palette.text
            }
        } else {
            Color {
                a: 0.12,
                ..palette.text
            }
        };

        text_input::Style {
            background: Background::Color(Color {
                a: 0.35,
                ..palette.background
            }),
            border: Border {
                radius: RADIUS_SMALL.into(),
                width: 1.0,
                color: border_color,
            },
            icon: palette.text_secondary,
            placeholder: palette.text_secondary,
            value: palette.text,
            // Cursor color and selection both pull from the accent.
            selection: Color {
                a: 0.3,
                ..palette.accent
            },
        }
    }
}

/// Style closure for the dropdown menu of a `combo_box`.
/// In iced 0.14, `menu::StyleFn` takes only `&Theme` — there is no per-item
/// hover state exposed here, so the "hovered item" visual is whatever
/// `selected_background` paints (iced highlights the row under the cursor
/// using the selected style). We tint that with the palette accent.
pub fn accent_combo_box_menu_style(palette: Palette) -> impl Fn(&Theme) -> menu::Style {
    move |_theme: &Theme| menu::Style {
        background: Background::Color(palette.surface),
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 1.0,
            color: Color {
                a: 0.15,
                ..palette.text
            },
        },
        text_color: palette.text,
        selected_text_color: palette.background,
        selected_background: Background::Color(Color {
            a: 0.9,
            ..palette.accent
        }),
        shadow: Shadow::default(),
    }
}
