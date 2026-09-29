use crate::config::{AppConfig, ShortcutId};
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, hint_text, section, shortcut_row};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let listening = |field| draft.recording == Some(field);

    let mut rows: Vec<Element<'a, Message>> = ShortcutId::ALL
        .into_iter()
        .map(|id| {
            let field = SettingsField::Shortcut(id);
            let value = draft
                .shortcuts
                .get(&id)
                .map(String::as_str)
                .unwrap_or_default();
            shortcut_row(id.label(), value, field, listening(field), palette)
        })
        .collect();
    rows.push(hint_text(crate::t!("settings.shortcuts.hint"), palette));

    let mut sections = vec![section(
        crate::t!("settings.shortcuts.application"),
        column(rows)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into(),
        palette,
    )];

    if !draft.plugin_shortcuts.is_empty() {
        let mut plugin_rows: Vec<Element<'a, Message>> = draft
            .plugin_shortcuts
            .iter()
            .enumerate()
            .map(|(index, row)| {
                let field = SettingsField::PluginShortcut(index);
                shortcut_row(&row.label, &row.binding, field, listening(field), palette)
            })
            .collect();
        plugin_rows.push(hint_text(
            crate::t!("settings.shortcuts.plugin_hint"),
            palette,
        ));
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
