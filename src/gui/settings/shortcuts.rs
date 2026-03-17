use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, hint_text, input_row, section};
use crate::gui::theme::SPACING_NORMAL;
use iced::widget::column;
use iced::{Element, Length};

pub fn view<'a>(_config: &'a AppConfig, draft: &'a SettingsDraft) -> Element<'a, Message> {
    let app_section = section(
        "Application",
        column(vec![
            input_row(
                "New tab",
                &draft.shortcut_new_tab,
                SettingsField::ShortcutNewTab,
            ),
            input_row(
                "Close tab",
                &draft.shortcut_close_tab,
                SettingsField::ShortcutCloseTab,
            ),
            input_row(
                "Open settings",
                &draft.shortcut_open_settings,
                SettingsField::ShortcutOpenSettings,
            ),
            input_row(
                "Next tab",
                &draft.shortcut_next_tab,
                SettingsField::ShortcutNextTab,
            ),
            input_row(
                "Previous tab",
                &draft.shortcut_prev_tab,
                SettingsField::ShortcutPrevTab,
            ),
            input_row("Quit", &draft.shortcut_quit, SettingsField::ShortcutQuit),
            hint_text("Format: Command+T, Ctrl+W, Ctrl+PageDown, Command+Comma"),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
    );

    column(vec![app_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}
