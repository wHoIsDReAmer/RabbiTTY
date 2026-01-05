use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::theme::{SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{column, row, text};
use iced::{Alignment, Element, Length};

pub mod terminal;
pub mod theme;
pub mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Ui,
    Terminal,
    Theme,
}

impl SettingsCategory {
    pub const ALL: [Self; 3] = [Self::Ui, Self::Terminal, Self::Theme];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ui => "UI",
            Self::Terminal => "Terminal",
            Self::Theme => "Theme",
        }
    }
}

pub fn view_category(category: SettingsCategory, config: &AppConfig) -> Element<'_, Message> {
    match category {
        SettingsCategory::Ui => ui::view(config),
        SettingsCategory::Terminal => terminal::view(config),
        SettingsCategory::Theme => theme::view(config),
    }
}

pub fn setting_row(label: &str, value: String) -> Element<'_, Message> {
    row![
        text(label).size(13),
        text(value).size(13),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

pub fn section<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    column(vec![text(title).size(15).into(), body])
        .spacing(SPACING_SMALL)
        .width(Length::Fill)
        .into()
}

pub fn format_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}
