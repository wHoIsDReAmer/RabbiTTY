use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{
    SettingsDraft, SettingsField, TerminalFontOption, hint_text, input_row_with_suffix, section,
};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::{column, pick_list, row, text};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    terminal_font_options: &'a [TerminalFontOption],
) -> Element<'a, Message> {
    let selected_font = terminal_font_options
        .iter()
        .find(|option| option.value == draft.terminal_font_selection)
        .cloned();

    let font_section = section(
        "Font",
        column(vec![
            input_row_with_suffix(
                "Size",
                &draft.terminal_font_size,
                SettingsField::TerminalFontSize,
                "pt",
            ),
            row![
                text("Font family").size(13).width(Length::Fixed(160.0)),
                pick_list(terminal_font_options, selected_font, |option| {
                    Message::SettingsInputChanged(
                        SettingsField::TerminalFontSelection,
                        option.value,
                    )
                })
                .placeholder("Select terminal font")
                .width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
            hint_text("Monospaced fonts are recommended for terminal text."),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    let padding_section = section(
        "Padding",
        column(vec![
            input_row_with_suffix(
                "Horizontal",
                &draft.terminal_padding_x,
                SettingsField::TerminalPaddingX,
                "px",
            ),
            input_row_with_suffix(
                "Vertical",
                &draft.terminal_padding_y,
                SettingsField::TerminalPaddingY,
                "px",
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    column(vec![font_section, padding_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
