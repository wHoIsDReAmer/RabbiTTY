use crate::config::{AppConfig, ShortcutId};
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, hint_text, input_row, section};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let mut rows: Vec<Element<'a, Message>> = ShortcutId::ALL
        .into_iter()
        .map(|id| {
            let value = draft
                .shortcuts
                .get(&id)
                .map(String::as_str)
                .unwrap_or_default();
            input_row(id.label(), value, SettingsField::Shortcut(id), palette)
        })
        .collect();
    rows.push(hint_text(
        "Format: Command+T, Ctrl+W, Ctrl+PageDown, Command+Comma",
        palette,
    ));

    let mut sections = vec![section(
        crate::t!("settings.shortcuts.application"),
        column(rows)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        palette,
    )];

    if !draft.plugin_shortcuts.is_empty() {
        let plugin_rows: Vec<Element<'a, Message>> = draft
            .plugin_shortcuts
            .iter()
            .enumerate()
            .map(|(index, row)| {
                input_row(
                    &row.label,
                    &row.binding,
                    SettingsField::PluginShortcut(index),
                    palette,
                )
            })
            .collect();
        sections.push(section(
            crate::t!("settings.shortcuts.plugins"),
            column(plugin_rows)
                .spacing(SPACING_NORMAL)
                .width(Length::Fill)
                .into(),
            palette,
        ));
    }

    column(sections)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
