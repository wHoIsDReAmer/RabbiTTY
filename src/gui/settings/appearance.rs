use crate::config::{AppConfig, CursorShape};
use crate::gui::app::Message;
use crate::gui::settings::{
    SettingsDraft, SettingsField, TerminalFontOption, hint_text, input_row_with_suffix, section,
    segmented_control,
};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use crate::i18n::AVAILABLE_LOCALES;
use iced::widget::{checkbox, column, combo_box, row, text};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    font_combo_state: &'a combo_box::State<TerminalFontOption>,
    show_all_fonts: bool,
    selected_font: Option<&'a TerminalFontOption>,
    palette: Palette,
) -> Element<'a, Message> {
    let language_section = section(
        t!("settings.language.section_title"),
        language_picker(&draft.language, palette),
        palette,
    );

    let font_section = section(
        crate::t!("settings.terminal.font_section"),
        column(vec![
            input_row_with_suffix(
                crate::t!("settings.terminal.size"),
                &draft.terminal_font_size,
                SettingsField::TerminalFontSize,
                "pt",
                palette,
            ),
            row![
                text(crate::t!("settings.terminal.font_family"))
                    .size(13)
                    .width(Length::Fixed(160.0)),
                combo_box(
                    font_combo_state,
                    crate::t!("settings.terminal.font_search_placeholder"),
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
                    .label(crate::t!("settings.terminal.show_all_fonts"))
                    .on_toggle(Message::ToggleShowAllFonts)
                    .size(14)
                    .text_size(13),
            ]
            .into(),
            hint_text(
                if draft.terminal_font_selection.is_empty() {
                    crate::t!("settings.terminal.font_hint_bundled")
                } else {
                    crate::t!("settings.terminal.font_hint_monospace")
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
        crate::t!("settings.terminal.padding_section"),
        column(vec![
            input_row_with_suffix(
                crate::t!("settings.terminal.horizontal"),
                &draft.terminal_padding_x,
                SettingsField::TerminalPaddingX,
                "px",
                palette,
            ),
            input_row_with_suffix(
                crate::t!("settings.terminal.vertical"),
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

    let cursor_section = section(
        crate::t!("settings.terminal.cursor_section"),
        segmented_control(
            crate::t!("settings.terminal.shape"),
            CursorShape::ALL
                .iter()
                .map(|&shape| {
                    (
                        cursor_shape_label(shape),
                        Message::SettingsCursorShapeSelected(shape),
                        draft.cursor_shape == shape,
                    )
                })
                .collect(),
            palette,
        ),
        palette,
    );

    column(vec![
        language_section,
        font_section,
        padding_section,
        cursor_section,
    ])
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn cursor_shape_label(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Block => crate::t!("settings.terminal.cursor_shape.block"),
        CursorShape::Bar => crate::t!("settings.terminal.cursor_shape.bar"),
        CursorShape::Underline => crate::t!("settings.terminal.cursor_shape.underline"),
    }
}

fn language_picker<'a>(current: &'a str, palette: Palette) -> Element<'a, Message> {
    let mut segments: Vec<(&'a str, Message, bool)> =
        Vec::with_capacity(AVAILABLE_LOCALES.len() + 1);
    segments.push((
        t!("settings.language.auto"),
        Message::SettingsInputCommitted(SettingsField::AppearanceLanguage, "auto".to_string()),
        current == "auto",
    ));
    for locale in AVAILABLE_LOCALES {
        segments.push((
            locale.native_label,
            Message::SettingsInputCommitted(
                SettingsField::AppearanceLanguage,
                locale.tag.to_string(),
            ),
            current == locale.tag,
        ));
    }
    segmented_control("", segments, palette)
}
