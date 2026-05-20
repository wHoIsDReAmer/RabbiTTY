use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::components::{
    accent_combo_box_input_style, accent_combo_box_menu_style, accent_toggler_style,
};
use crate::gui::settings::{
    SettingsDraft, SettingsField, TerminalFontOption, hint_text, input_row_with_suffix, section,
    segmented_control,
};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use crate::i18n::AVAILABLE_LOCALES;
use iced::widget::{checkbox, column, combo_box, row, text, toggler};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    config: &'a AppConfig,
    draft: &'a SettingsDraft,
    font_combo_state: &'a combo_box::State<TerminalFontOption>,
    show_all_fonts: bool,
    selected_font: Option<&'a TerminalFontOption>,
    palette: Palette,
) -> Element<'a, Message> {
    let language_section = section(
        t!("settings.language.section_title"),
        language_picker(&draft.language, palette, config.ui.animations_enabled),
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
                .width(Length::Fill)
                .input_style(accent_combo_box_input_style(palette))
                .menu_style(accent_combo_box_menu_style(palette)),
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

    let animations_section = section(
        crate::t!("settings.appearance.animations_section"),
        row![
            text(crate::t!("settings.appearance.animations"))
                .size(13)
                .width(Length::Fixed(160.0)),
            toggler(draft.animations_enabled)
                .on_toggle(Message::SettingsAnimationsToggled)
                .size(18)
                .style(accent_toggler_style(palette)),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    column(vec![
        language_section,
        animations_section,
        font_section,
        padding_section,
    ])
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn language_picker<'a>(
    current: &'a str,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
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
    segmented_control("", segments, palette, animations_enabled)
}
