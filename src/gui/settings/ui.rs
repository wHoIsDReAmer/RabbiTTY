use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, input_row, section, SettingsField};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let window_section = section(
        "Window",
        column(vec![
            input_row(
                "Window width",
                &draft.window_width,
                SettingsField::UiWindowWidth,
            ),
            input_row(
                "Window height",
                &draft.window_height,
                SettingsField::UiWindowHeight,
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
