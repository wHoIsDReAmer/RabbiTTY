use crate::config::{AppConfig, BellMode, CursorShape, RightClickAction};
use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::accent_toggler_style;
use crate::gui::settings::{
    SettingsDraft, SettingsField, input_row_with_suffix, section, segmented_control,
};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use iced::widget::{column, row, text, toggler};
use iced::{Alignment, Element, Length};

pub fn view<'a>(
    config: &'a AppConfig,
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
                    .on_toggle(|a0| Message::Settings(SettingsMessage::BracketedPasteToggled(a0)))
                    .size(18)
                    .style(accent_toggler_style(palette)),
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
                    .on_toggle(|a0| Message::Settings(
                        SettingsMessage::MultilinePasteConfirmToggled(a0)
                    ))
                    .size(18)
                    .style(accent_toggler_style(palette)),
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
        column(vec![
            segmented_control(
                crate::t!("settings.terminal.shape"),
                CursorShape::ALL
                    .iter()
                    .map(|&shape| {
                        (
                            cursor_shape_label(shape),
                            Message::Settings(SettingsMessage::CursorShapeSelected(shape)),
                            draft.cursor_shape == shape,
                        )
                    })
                    .collect(),
                palette,
                config.ui.animations_enabled,
            ),
            row![
                text(crate::t!("settings.terminal.blink"))
                    .size(13)
                    .width(label_width),
                toggler(draft.cursor_blink)
                    .on_toggle(|a0| Message::Settings(SettingsMessage::CursorBlinkToggled(a0)))
                    .size(18)
                    .style(accent_toggler_style(palette)),
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

    let colors_section = section(
        crate::t!("settings.terminal.colors_section"),
        row![
            text(crate::t!("settings.terminal.bold_is_bright"))
                .size(13)
                .width(label_width),
            toggler(draft.bold_is_bright)
                .on_toggle(|a0| Message::Settings(SettingsMessage::BoldIsBrightToggled(a0)))
                .size(18)
                .style(accent_toggler_style(palette)),
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
                        Message::Settings(SettingsMessage::BellModeSelected(mode)),
                        draft.bell_mode == mode,
                    )
                })
                .collect(),
            palette,
            config.ui.animations_enabled,
        ),
        palette,
    );

    let mouse_section = section(
        crate::t!("settings.terminal.mouse_section"),
        segmented_control(
            crate::t!("settings.terminal.right_click"),
            RightClickAction::ALL
                .iter()
                .map(|&action| {
                    (
                        right_click_action_label(action),
                        Message::Settings(SettingsMessage::RightClickActionSelected(action)),
                        draft.right_click_action == action,
                    )
                })
                .collect(),
            palette,
            config.ui.animations_enabled,
        ),
        palette,
    );

    column(vec![
        scrollback_section,
        paste_section,
        cursor_section,
        colors_section,
        bell_section,
        mouse_section,
    ])
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

/// Glyphs that visually represent each cursor shape in the segmented control.
fn cursor_shape_label(shape: CursorShape) -> &'static str {
    match shape {
        CursorShape::Block => "█",
        CursorShape::Bar => "▎",
        CursorShape::Underline => "▁",
    }
}

fn bell_mode_label(mode: BellMode) -> &'static str {
    match mode {
        BellMode::Off => crate::t!("settings.terminal.bell_mode.off"),
        BellMode::Visual => crate::t!("settings.terminal.bell_mode.visual"),
        BellMode::Sound => crate::t!("settings.terminal.bell_mode.sound"),
    }
}

fn right_click_action_label(action: RightClickAction) -> &'static str {
    match action {
        RightClickAction::Paste => crate::t!("settings.terminal.right_click_action.paste"),
        RightClickAction::Menu => crate::t!("settings.terminal.right_click_action.menu"),
        RightClickAction::None => crate::t!("settings.terminal.right_click_action.none"),
    }
}
