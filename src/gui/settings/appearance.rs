use crate::config::{AppConfig, TabBarPosition};
use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::{
    accent_combo_box_input_style, accent_combo_box_menu_style, accent_pick_list_style,
    accent_toggler_style,
};
use crate::gui::settings::{
    ROW_SPACING, SECTION_SPACING, SettingsDraft, SettingsField, TerminalFontOption, hint_text,
    input_row_with_suffix, section, segmented_control, setting_row,
};
use crate::gui::theme::Palette;
use crate::i18n::AVAILABLE_LOCALES;
use iced::widget::{checkbox, column, combo_box, pick_list, row, toggler};
use iced::{Element, Length};

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
            setting_row(
                crate::t!("settings.terminal.font_family"),
                combo_box(
                    font_combo_state,
                    crate::t!("settings.terminal.font_search_placeholder"),
                    selected_font,
                    |a0| Message::Settings(SettingsMessage::FontSelected(a0)),
                )
                .width(Length::Fixed(260.0))
                .input_style(accent_combo_box_input_style(palette))
                .menu_style(accent_combo_box_menu_style(palette)),
                palette,
            ),
            row![
                checkbox(show_all_fonts)
                    .label(crate::t!("settings.terminal.show_all_fonts"))
                    .on_toggle(|a0| Message::Settings(SettingsMessage::ToggleShowAllFonts(a0)))
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
        .spacing(ROW_SPACING)
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
        .spacing(ROW_SPACING)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let animations_section = section(
        crate::t!("settings.appearance.animations_section"),
        setting_row(
            crate::t!("settings.appearance.animations"),
            toggler(draft.animations_enabled)
                .on_toggle(|a0| Message::Settings(SettingsMessage::AnimationsToggled(a0)))
                .size(18)
                .style(accent_toggler_style(palette)),
            palette,
        ),
        palette,
    );

    let tabs_section = section(
        crate::t!("settings.appearance.tabs_section"),
        segmented_control(
            crate::t!("settings.appearance.position"),
            TabBarPosition::ALL
                .iter()
                .map(|&pos| {
                    (
                        tab_bar_position_label(pos),
                        Message::Settings(SettingsMessage::TabBarPositionSelected(pos)),
                        draft.tab_bar_position == pos,
                    )
                })
                .collect(),
            palette,
            config.ui.animations_enabled,
        ),
        palette,
    );

    column(vec![
        language_section,
        animations_section,
        tabs_section,
        font_section,
        padding_section,
    ])
    .spacing(SECTION_SPACING)
    .width(Length::Fill)
    .into()
}

fn tab_bar_position_label(position: TabBarPosition) -> &'static str {
    match position {
        TabBarPosition::Top => crate::t!("settings.appearance.tab_position.top"),
        TabBarPosition::Bottom => crate::t!("settings.appearance.tab_position.bottom"),
    }
}

#[derive(Clone, PartialEq, Eq)]
struct LanguageOption {
    tag: &'static str,
    label: &'static str,
}

impl std::fmt::Display for LanguageOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label)
    }
}

fn language_picker<'a>(current: &'a str, palette: Palette) -> Element<'a, Message> {
    let mut options: Vec<LanguageOption> = Vec::with_capacity(AVAILABLE_LOCALES.len() + 1);
    options.push(LanguageOption {
        tag: "auto",
        label: t!("settings.language.auto"),
    });
    for locale in AVAILABLE_LOCALES {
        options.push(LanguageOption {
            tag: locale.tag,
            label: locale.native_label,
        });
    }
    let selected = options.iter().find(|o| o.tag == current).cloned();

    setting_row(
        "",
        pick_list(options, selected, |option: LanguageOption| {
            Message::Settings(SettingsMessage::InputCommitted(
                SettingsField::AppearanceLanguage,
                option.tag.to_string(),
            ))
        })
        .width(Length::Fixed(180.0))
        .style(accent_pick_list_style(palette))
        .menu_style(accent_combo_box_menu_style(palette)),
        palette,
    )
}
