use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{format_rgb, section, setting_row};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view(config: &AppConfig) -> Element<'_, Message> {
    let colors_section = section(
        "Colors",
        column(vec![
            setting_row("Foreground", format_rgb(config.theme.foreground)),
            setting_row("Background", format_rgb(config.theme.background)),
            setting_row("Cursor", format_rgb(config.theme.cursor)),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let opacity_section = section(
        "Opacity",
        column(vec![setting_row(
            "Background opacity",
            format!("{:.2}", config.theme.background_opacity),
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
