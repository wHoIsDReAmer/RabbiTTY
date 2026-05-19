use crate::config::{AppConfig, BellMode};
use crate::gui::app::Message;
use crate::gui::settings::{
    SettingsDraft, SettingsField, input_row_with_suffix, section, segmented_control,
};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use iced::widget::{column, row, text, toggler};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let scrollback_section = section(
        crate::t!("settings.terminal.scrolling_section"),
        column(vec![
            input_row_with_suffix(
                crate::t!("settings.terminal.scrollback"),
                &draft.terminal_scrollback,
                SettingsField::TerminalScrollback,
                crate::t!("settings.terminal.scrollback_suffix"),
                palette,
            ),
            input_row_with_suffix(
                crate::t!("settings.terminal.scroll_speed"),
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
        crate::t!("settings.terminal.paste_section"),
        column(vec![
            row![
                text(crate::t!("settings.terminal.bracketed_paste"))
                    .size(13)
                    .width(label_width),
                toggler(draft.bracketed_paste)
                    .on_toggle(Message::SettingsBracketedPasteToggled)
                    .size(18),
            ]
            .align_y(Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
            row![
                text(crate::t!("settings.terminal.confirm_multiline_paste"))
                    .size(13)
                    .width(label_width),
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
        crate::t!("settings.terminal.cursor_section"),
        row![
            text(crate::t!("settings.terminal.blink"))
                .size(13)
                .width(label_width),
            toggler(draft.cursor_blink)
                .on_toggle(Message::SettingsCursorBlinkToggled)
                .size(18),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    let bell_section = section(
        crate::t!("settings.terminal.bell_section"),
        segmented_control(
            crate::t!("settings.terminal.behavior"),
            BellMode::ALL
                .iter()
                .map(|&mode| {
                    (
                        bell_mode_label(mode),
                        Message::SettingsBellModeSelected(mode),
                        draft.bell_mode == mode,
                    )
                })
                .collect(),
            palette,
        ),
        palette,
    );

    column(vec![
        scrollback_section,
        paste_section,
        cursor_section,
        bell_section,
    ])
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn bell_mode_label(mode: BellMode) -> &'static str {
    match mode {
        BellMode::Off => crate::t!("settings.terminal.bell_mode.off"),
        BellMode::Visual => crate::t!("settings.terminal.bell_mode.visual"),
        BellMode::Sound => crate::t!("settings.terminal.bell_mode.sound"),
    }
}
