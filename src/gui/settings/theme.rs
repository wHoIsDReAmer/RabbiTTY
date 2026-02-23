use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, input_row, section, toggle_row};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::{column, text};
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let colors_section = section(
        "Colors",
        column(vec![
            input_row(
                "Foreground",
                &draft.foreground,
                SettingsField::ThemeForeground,
            ),
            input_row(
                "Background",
                &draft.background,
                SettingsField::ThemeBackground,
            ),
            input_row("Cursor", &draft.cursor, SettingsField::ThemeCursor),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let opacity_section = section(
        "Opacity",
        column(vec![input_row(
            "Background opacity",
            &draft.background_opacity,
            SettingsField::ThemeBackgroundOpacity,
        )])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let blur_section = section(
        "Blur",
        column(vec![
            toggle_row("Enable blur", draft.blur_enabled),
            text("On macOS, changing this requires restart.")
                .size(12)
                .into(),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    #[cfg(target_os = "macos")]
    let macos_blur_section = section(
        "macOS Blur",
        column(vec![
            input_row(
                "Material (sidebar/menu/titlebar/...)",
                &draft.macos_blur_material,
                SettingsField::ThemeMacosBlurMaterial,
            ),
            input_row(
                "Alpha (0.0-1.0)",
                &draft.macos_blur_alpha,
                SettingsField::ThemeMacosBlurAlpha,
            ),
            text("Material controls style family; alpha controls intensity.")
                .size(12)
                .into(),
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
