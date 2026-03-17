use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, input_row_with_suffix, section};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let window_section = section(
        "Window",
        column(vec![
            input_row_with_suffix(
                "Width",
                &draft.window_width,
                SettingsField::UiWindowWidth,
                "px",
            ),
            input_row_with_suffix(
                "Height",
                &draft.window_height,
                SettingsField::UiWindowHeight,
                "px",
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    column(vec![window_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
