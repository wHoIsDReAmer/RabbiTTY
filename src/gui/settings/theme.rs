use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{
    SettingsDraft, SettingsField, color_input_row, hint_text, input_row_with_suffix, section,
    toggle_row,
};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let colors_section = section(
        "Colors",
        column(vec![
            color_input_row(
                "Foreground",
                &draft.foreground,
                SettingsField::ThemeForeground,
            ),
            color_input_row(
                "Background",
                &draft.background,
                SettingsField::ThemeBackground,
            ),
            color_input_row("Cursor", &draft.cursor, SettingsField::ThemeCursor),
            hint_text("Hex format: #rrggbb"),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let opacity_section = section(
        "Opacity",
        column(vec![input_row_with_suffix(
            "Background opacity",
            &draft.background_opacity,
            SettingsField::ThemeBackgroundOpacity,
            "0.0 ~ 1.0",
        )])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let blur_section = section(
        "Blur",
        column(vec![
            toggle_row("Enable blur", draft.blur_enabled),
            hint_text("On macOS, changing this requires restart."),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    #[cfg(target_os = "macos")]
    let macos_blur_section = section(
        "macOS Blur",
        column(vec![
            input_row_with_suffix(
                "Material",
                &draft.macos_blur_material,
                SettingsField::ThemeMacosBlurMaterial,
                "sidebar / menu / titlebar",
            ),
            input_row_with_suffix(
                "Alpha",
                &draft.macos_blur_alpha,
                SettingsField::ThemeMacosBlurAlpha,
                "0.0 ~ 1.0",
            ),
            hint_text("Material controls style family; alpha controls intensity."),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    #[cfg(target_os = "macos")]
    let sections = vec![
        colors_section,
        opacity_section,
        blur_section,
        macos_blur_section,
    ];

    #[cfg(not(target_os = "macos"))]
    let sections = vec![colors_section, opacity_section, blur_section];

    column(sections)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
