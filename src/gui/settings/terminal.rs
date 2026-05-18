use crate::config::{AppConfig, BellMode, CursorShape};
use crate::gui::app::Message;
use crate::gui::settings::{
    SettingsDraft, SettingsField, TerminalFontOption, hint_text, input_row_with_suffix, section,
};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use iced::widget::{checkbox, column, combo_box, pick_list, row, text, toggler};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    font_combo_state: &'a combo_box::State<TerminalFontOption>,
    show_all_fonts: bool,
    selected_font: Option<&'a TerminalFontOption>,
    palette: Palette,
) -> Element<'a, Message> {
    let font_section = section(
        "Font",
        column(vec![
            input_row_with_suffix(
                "Size",
                &draft.terminal_font_size,
                SettingsField::TerminalFontSize,
                "pt",
                palette,
            ),
            row![
                text("Font family").size(13).width(Length::Fixed(160.0)),
                combo_box(
                    font_combo_state,
                    "Search fonts...",
                    selected_font,
                    Message::FontSelected,
                )
                .width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
            row![
                checkbox(show_all_fonts)
                    .label("Show all fonts")
                    .on_toggle(Message::ToggleShowAllFonts)
                    .size(14)
                    .text_size(13),
            ]
            .into(),
            hint_text(
                if draft.terminal_font_selection.is_empty() {
                    "Using bundled DejaVu Sans Mono."
                } else {
                    "Monospaced fonts are recommended for terminal text."
                },
                palette,
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let padding_section = section(
        "Padding",
        column(vec![
            input_row_with_suffix(
                "Horizontal",
                &draft.terminal_padding_x,
                SettingsField::TerminalPaddingX,
                "px",
                palette,
            ),
            input_row_with_suffix(
                "Vertical",
                &draft.terminal_padding_y,
                SettingsField::TerminalPaddingY,
                "px",
                palette,
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let scrollback_section = section(
        "Scrolling",
        column(vec![
            input_row_with_suffix(
                "Scrollback",
                &draft.terminal_scrollback,
                SettingsField::TerminalScrollback,
                "lines",
                palette,
            ),
            input_row_with_suffix(
                "Scroll speed",
                &draft.terminal_scroll_speed,
                SettingsField::TerminalScrollSpeed,
                "x",
                palette,
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let label_width = Length::Fixed(160.0);

    let paste_section = section(
        "Paste",
        column(vec![
            row![
                text("Bracketed paste").size(13).width(label_width),
                toggler(draft.bracketed_paste)
                    .on_toggle(Message::SettingsBracketedPasteToggled)
                    .size(18),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
            row![
                text("Confirm multi-line paste").size(13).width(label_width),
                toggler(draft.multiline_paste_confirm)
                    .on_toggle(Message::SettingsMultilinePasteConfirmToggled)
                    .size(18),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let cursor_section = section(
        "Cursor",
        column(vec![
            row![
                text("Shape").size(13).width(label_width),
                pick_list(
                    CursorShape::ALL,
                    Some(draft.cursor_shape),
                    Message::SettingsCursorShapeSelected,
                )
                .text_size(13),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
            row![
                text("Blink").size(13).width(label_width),
                toggler(draft.cursor_blink)
                    .on_toggle(Message::SettingsCursorBlinkToggled)
                    .size(18),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let bell_section = section(
        "Bell",
        column(vec![
            row![
                text("Behavior").size(13).width(label_width),
                pick_list(
                    BellMode::ALL,
                    Some(draft.bell_mode),
                    Message::SettingsBellModeSelected,
                )
                .text_size(13),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    column(vec![
        font_section,
        padding_section,
        scrollback_section,
        cursor_section,
        bell_section,
        paste_section,
    ])
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}
