use crate::config::{AppConfig, AppConfigUpdates, parse_hex_color};
use crate::gui::app::Message;
use crate::gui::theme::{SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{column, row, text, text_input};
use iced::{Alignment, Element, Length};

pub mod terminal;
pub mod theme;
pub mod ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    UiWindowWidth,
    UiWindowHeight,
    TerminalCellWidth,
    TerminalCellHeight,
    ThemeForeground,
    ThemeBackground,
    ThemeCursor,
    ThemeBackgroundOpacity,
}

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

#[derive(Debug, Clone)]
pub struct SettingsDraft {
    pub window_width: String,
    pub window_height: String,
    pub cell_width: String,
    pub cell_height: String,
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub background_opacity: String,
}

impl SettingsDraft {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            window_width: format!("{:.0}", config.ui.window_width),
            window_height: format!("{:.0}", config.ui.window_height),
            cell_width: format!("{:.1}", config.terminal.cell_width),
            cell_height: format!("{:.1}", config.terminal.cell_height),
            foreground: format_rgb(config.theme.foreground),
            background: format_rgb(config.theme.background),
            cursor: format_rgb(config.theme.cursor),
            background_opacity: format!("{:.2}", config.theme.background_opacity),
        }
    }

    pub fn update(&mut self, field: SettingsField, value: String) {
        match field {
            SettingsField::UiWindowWidth => self.window_width = value,
            SettingsField::UiWindowHeight => self.window_height = value,
            SettingsField::TerminalCellWidth => self.cell_width = value,
            SettingsField::TerminalCellHeight => self.cell_height = value,
            SettingsField::ThemeForeground => self.foreground = value,
            SettingsField::ThemeBackground => self.background = value,
            SettingsField::ThemeCursor => self.cursor = value,
            SettingsField::ThemeBackgroundOpacity => self.background_opacity = value,
        }
    }

    pub fn to_updates(&self) -> AppConfigUpdates {
        let mut updates = AppConfigUpdates::default();
        updates.window_width = parse_f32(&self.window_width);
        updates.window_height = parse_f32(&self.window_height);
        updates.cell_width = parse_f32(&self.cell_width);
        updates.cell_height = parse_f32(&self.cell_height);
        updates.foreground = parse_hex_color(&self.foreground);
        updates.background = parse_hex_color(&self.background);
        updates.cursor = parse_hex_color(&self.cursor);
        updates.background_opacity = parse_f32(&self.background_opacity);
        updates
    }
}

fn parse_f32(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok()
}

pub fn view_category<'a>(
    category: SettingsCategory,
    config: &'a AppConfig,
    draft: &'a SettingsDraft,
) -> Element<'a, Message> {
    match category {
        SettingsCategory::Ui => ui::view(config, draft),
        SettingsCategory::Terminal => terminal::view(config, draft),
        SettingsCategory::Theme => theme::view(config, draft),
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

pub fn input_row<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
) -> Element<'a, Message> {
    row![
        text(label).size(13),
        text_input("", value)
            .on_input(move |next| Message::SettingsInputChanged(field, next))
            .padding(6)
            .width(Length::Fixed(140.0)),
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
