use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, input_row, section};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let terminal_section = section(
        "Cells",
        column(vec![
            input_row(
                "Cell width",
                &draft.cell_width,
                SettingsField::TerminalCellWidth,
            ),
            input_row(
                "Cell height",
                &draft.cell_height,
                SettingsField::TerminalCellHeight,
            ),
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
