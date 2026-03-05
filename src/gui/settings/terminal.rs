use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, TerminalFontOption, input_row, section};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::{column, pick_list, text};
use iced::{Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    terminal_font_options: &'a [TerminalFontOption],
) -> Element<'a, Message> {
    let selected_font = terminal_font_options
        .iter()
        .find(|option| option.value == draft.terminal_font_selection)
        .cloned();

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
            input_row(
                "Terminal font size",
                &draft.terminal_font_size,
                SettingsField::TerminalFontSize,
            ),
            column(vec![
                text("Terminal font").size(13).into(),
                pick_list(terminal_font_options, selected_font, |option| {
                    Message::SettingsInputChanged(
                        SettingsField::TerminalFontSelection,
                        option.value,
                    )
                })
                .placeholder("Select terminal font")
                .width(Length::Fill)
                .into(),
                text("Terminal text looks best with monospaced fonts.")
                    .size(12)
                    .into(),
            ])
            .spacing(6)
            .into(),
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
