use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, input_row, section};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
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

    column(vec![colors_section, opacity_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
