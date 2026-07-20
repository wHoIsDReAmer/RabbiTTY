use crate::config::AppConfig;
use crate::gui::app::{Message, SettingsMessage};
use crate::gui::settings::{
    SettingsDraft, SettingsField, format_rgb, hint_text, input_row_with_suffix, section, toggle_row,
};
use crate::gui::theme::{Palette, RADIUS_SMALL, SPACING_NORMAL};
use crate::terminal::theme::{ColorPreset, all_presets};
use iced::widget::{Column, Row, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length};

const LABEL_WIDTH: f32 = 160.0;

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    // -- Preset picker: 2-column grid of visual cards --
    let grid_rows = build_preset_grid(draft, &palette);

    let presets_section = section(
        crate::t!("settings.theme.color_scheme_section"),
        Column::with_children(grid_rows)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        palette,
    );

    // -- Color palette pickers for fg/bg/cursor --
    let current_preset = crate::terminal::theme::find_preset(&draft.color_scheme);
    let colors_section = section(
        crate::t!("settings.theme.colors_section"),
        column(vec![
            color_palette_row(
                crate::t!("settings.theme.foreground"),
                &draft.foreground,
                SettingsField::ThemeForeground,
                current_preset,
                &palette,
            ),
            color_palette_row(
                crate::t!("settings.theme.background"),
                &draft.background,
                SettingsField::ThemeBackground,
                current_preset,
                &palette,
            ),
            color_palette_row(
                crate::t!("settings.theme.cursor"),
                &draft.cursor,
                SettingsField::ThemeCursor,
                current_preset,
                &palette,
            ),
            hint_text(crate::t!("settings.theme.colors_hint"), palette),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let opacity_section = section(
        crate::t!("settings.theme.opacity_section"),
        column(vec![input_row_with_suffix(
            crate::t!("settings.theme.background_opacity"),
            &draft.background_opacity,
            SettingsField::ThemeBackgroundOpacity,
            "0.0 ~ 1.0",
            palette,
        )])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let blur_section = section(
        crate::t!("settings.theme.blur_section"),
        column(vec![toggle_row(
            crate::t!("settings.theme.enable_blur"),
            draft.blur_enabled,
            palette,
        )])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    #[cfg(target_os = "macos")]
    let macos_blur_section = section(
        crate::t!("settings.theme.macos_blur_section"),
        column(vec![
            input_row_with_suffix(
                crate::t!("settings.theme.blur_radius"),
                &draft.macos_blur_radius,
                SettingsField::ThemeMacosBlurRadius,
                "0 ~ 100",
                palette,
            ),
            hint_text(crate::t!("settings.theme.blur_radius_hint"), palette),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    #[cfg(target_os = "macos")]
    let sections = vec![
        presets_section,
        colors_section,
        opacity_section,
        blur_section,
        macos_blur_section,
    ];

    #[cfg(not(target_os = "macos"))]
    let sections = vec![
        presets_section,
        colors_section,
        opacity_section,
        blur_section,
    ];

    column(sections)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}

/// Build preset cards in a 2-column grid.
fn build_preset_grid<'a>(draft: &'a SettingsDraft, palette: &Palette) -> Vec<Element<'a, Message>> {
    let cards: Vec<Element<'a, Message>> = all_presets()
        .iter()
        .map(|preset| build_preset_card(preset, draft, palette))
        .collect();

    let mut rows = Vec::new();
    let mut iter = cards.into_iter();
    loop {
        match (iter.next(), iter.next()) {
            (Some(a), Some(b)) => {
                rows.push(
                    row![a, b]
                        .spacing(SPACING_NORMAL)
                        .width(Length::Fill)
                        .into(),
                );
            }
            (Some(a), None) => {
                rows.push(
                    row![a, container("").width(Length::Fill)]
                        .spacing(SPACING_NORMAL)
                        .width(Length::Fill)
                        .into(),
                );
            }
            _ => break,
        }
    }
    rows
}

fn build_preset_card<'a>(
    preset: &'static ColorPreset,
    draft: &'a SettingsDraft,
    palette: &Palette,
) -> Element<'a, Message> {
    let is_selected = draft.color_scheme.eq_ignore_ascii_case(&preset.name);

    // Color swatches: bg, fg, then 6 ANSI colors
    let swatches: Vec<Element<'a, Message>> = std::iter::once(preset.bg)
        .chain(std::iter::once(preset.fg))
        .chain(preset.ansi[1..7].iter().copied())
        .map(|c| color_dot(c, 14.0))
        .collect();

    let swatch_row = Row::with_children(swatches).spacing(3);

    let card_bg = Color::from_rgb8(preset.bg[0], preset.bg[1], preset.bg[2]);
    let card_fg = Color::from_rgb8(preset.fg[0], preset.fg[1], preset.fg[2]);

    let accent = palette.accent;
    let text_color = palette.text;
    let border_color = if is_selected {
        accent
    } else {
        Color {
            a: 0.15,
            ..text_color
        }
    };
    let border_width = if is_selected { 2.0 } else { 1.0 };

    let name = &preset.name;
    let card_content = column![text(name.as_str()).size(12).color(card_fg), swatch_row]
        .spacing(6)
        .width(Length::Fill);

    button(
        container(card_content)
            .padding([10, 12])
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(card_bg)),
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: border_width,
                    color: border_color,
                },
                ..Default::default()
            }),
    )
    .on_press(Message::Settings(SettingsMessage::InputCommitted(
        SettingsField::ThemeColorScheme,
        name.to_string(),
    )))
    .padding(0)
    .style(move |_theme: &iced::Theme, _status| button::Style {
        background: None,
        text_color: Color::TRANSPARENT,
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Default::default(),
        snap: true,
    })
    .width(Length::Fill)
    .into()
}

/// A color field with current swatch, palette of clickable options, and hex input.
fn color_palette_row<'a>(
    label: &'a str,
    current_hex: &'a str,
    field: SettingsField,
    preset: Option<&'static ColorPreset>,
    palette: &Palette,
) -> Element<'a, Message> {
    let parsed = crate::config::parse_hex_color(current_hex);
    let current_color = parsed
        .map(|rgb| Color::from_rgb8(rgb[0], rgb[1], rgb[2]))
        .unwrap_or(palette.error);

    // Build palette of clickable colors from the preset
    let palette_colors = build_palette_options(preset);

    let swatches: Vec<Element<'a, Message>> = palette_colors
        .iter()
        .map(|&c| {
            let is_current = parsed.is_some_and(|p| p == c);
            let hex = format_rgb(c);
            let dot_color = Color::from_rgb8(c[0], c[1], c[2]);
            let accent = palette.accent;
            let text_col = palette.text;

            button(
                container("")
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .style(move |_theme: &iced::Theme| {
                        let border_color = if is_current {
                            accent
                        } else {
                            Color { a: 0.2, ..text_col }
                        };
                        container::Style {
                            background: Some(Background::Color(dot_color)),
                            border: Border {
                                radius: 4.0.into(),
                                width: if is_current { 2.0 } else { 1.0 },
                                color: border_color,
                            },
                            ..Default::default()
                        }
                    }),
            )
            .on_press(Message::Settings(SettingsMessage::InputCommitted(
                field, hex,
            )))
            .padding(0)
            .style(|_theme: &iced::Theme, _status| button::Style {
                background: None,
                text_color: Color::TRANSPARENT,
                border: Border::default(),
                shadow: Default::default(),
                snap: true,
            })
            .into()
        })
        .collect();

    let swatch_grid = Row::with_children(swatches).spacing(4).width(Length::Fill);

    // Current color indicator + hex input
    let commit_msg = Message::Settings(SettingsMessage::InputCommitted(
        field,
        current_hex.to_owned(),
    ));
    let hex_input = crate::gui::settings::styled_text_input_small(
        current_hex,
        move |next| Message::Settings(SettingsMessage::InputChanged(field, next)),
        *palette,
    )
    .on_submit(commit_msg);

    column![
        row![
            text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
            container("")
                .width(Length::Fixed(20.0))
                .height(Length::Fixed(20.0))
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(current_color)),
                    border: Border {
                        radius: 4.0.into(),
                        width: 1.0,
                        color: Color {
                            a: 0.25,
                            ..Color::WHITE
                        },
                    },
                    ..Default::default()
                }),
            hex_input,
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill),
        row![container("").width(Length::Fixed(LABEL_WIDTH)), swatch_grid,]
            .spacing(SPACING_NORMAL)
            .width(Length::Fill),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

/// Build a unique set of palette colors from the current preset.
fn build_palette_options(preset: Option<&'static ColorPreset>) -> Vec<[u8; 3]> {
    let Some(preset) = preset else {
        return vec![];
    };

    let mut colors: Vec<[u8; 3]> = Vec::with_capacity(20);
    colors.push(preset.fg);
    colors.push(preset.bg);
    colors.push(preset.cursor);
    for c in &preset.ansi {
        if !colors.contains(c) {
            colors.push(*c);
        }
    }
    colors
}

/// Small color dot container.
fn color_dot<'a>(c: [u8; 3], size: f32) -> Element<'a, Message> {
    container("")
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb8(c[0], c[1], c[2]))),
            border: Border {
                radius: (size / 4.0).into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        })
        .into()
}
