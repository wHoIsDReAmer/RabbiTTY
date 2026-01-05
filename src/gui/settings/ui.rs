use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{section, setting_row};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view(config: &AppConfig) -> Element<'_, Message> {
    let window_section = section(
        "Window",
        column(vec![
            setting_row("Window width", format!("{:.0}", config.ui.window_width)),
            setting_row("Window height", format!("{:.0}", config.ui.window_height)),
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
