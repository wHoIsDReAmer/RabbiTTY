use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{section, setting_row};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view(config: &AppConfig) -> Element<'_, Message> {
    let terminal_section = section(
        "Cells",
        column(vec![
            setting_row("Cell width", format!("{:.1}", config.terminal.cell_width)),
            setting_row("Cell height", format!("{:.1}", config.terminal.cell_height)),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    column(vec![terminal_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
