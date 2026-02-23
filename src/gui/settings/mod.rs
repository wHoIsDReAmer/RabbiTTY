use crate::config::{AppConfig, AppConfigUpdates, parse_hex_color};
use crate::gui::app::Message;
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{column, container, row, text, text_input, toggler};
use iced::{Alignment, Background, Border, Color, Element, Length};

pub mod shortcuts;
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
    ThemeMacosBlurMaterial,
    ThemeMacosBlurAlpha,
    ShortcutNewTab,
    ShortcutCloseTab,
    ShortcutOpenSettings,
    ShortcutNextTab,
    ShortcutPrevTab,
    ShortcutQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Ui,
    Terminal,
    Theme,
    Shortcuts,
}

impl SettingsCategory {
    pub const ALL: [Self; 4] = [Self::Ui, Self::Terminal, Self::Theme, Self::Shortcuts];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ui => "UI",
            Self::Terminal => "Terminal",
            Self::Theme => "Theme",
            Self::Shortcuts => "Shortcuts",
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
    pub blur_enabled: bool,
    pub macos_blur_material: String,
    pub macos_blur_alpha: String,
    pub shortcut_new_tab: String,
    pub shortcut_close_tab: String,
    pub shortcut_open_settings: String,
    pub shortcut_next_tab: String,
    pub shortcut_prev_tab: String,
    pub shortcut_quit: String,
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
            blur_enabled: config.theme.blur_enabled,
            macos_blur_material: config.theme.macos_blur_material.clone(),
            macos_blur_alpha: format!("{:.2}", config.theme.macos_blur_alpha),
            shortcut_new_tab: config.shortcuts.new_tab.clone(),
            shortcut_close_tab: config.shortcuts.close_tab.clone(),
            shortcut_open_settings: config.shortcuts.open_settings.clone(),
            shortcut_next_tab: config.shortcuts.next_tab.clone(),
            shortcut_prev_tab: config.shortcuts.prev_tab.clone(),
            shortcut_quit: config.shortcuts.quit.clone(),
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
            SettingsField::ThemeMacosBlurMaterial => self.macos_blur_material = value,
            SettingsField::ThemeMacosBlurAlpha => self.macos_blur_alpha = value,
            SettingsField::ShortcutNewTab => self.shortcut_new_tab = value,
            SettingsField::ShortcutCloseTab => self.shortcut_close_tab = value,
            SettingsField::ShortcutOpenSettings => self.shortcut_open_settings = value,
            SettingsField::ShortcutNextTab => self.shortcut_next_tab = value,
            SettingsField::ShortcutPrevTab => self.shortcut_prev_tab = value,
            SettingsField::ShortcutQuit => self.shortcut_quit = value,
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
        updates.blur_enabled = Some(self.blur_enabled);
        if !self.macos_blur_material.trim().is_empty() {
            updates.macos_blur_material = Some(self.macos_blur_material.clone());
        }
        updates.macos_blur_alpha = parse_f32(&self.macos_blur_alpha);
        if !self.shortcut_new_tab.trim().is_empty() {
            updates.shortcut_new_tab = Some(self.shortcut_new_tab.clone());
        }
        if !self.shortcut_close_tab.trim().is_empty() {
            updates.shortcut_close_tab = Some(self.shortcut_close_tab.clone());
        }
        if !self.shortcut_open_settings.trim().is_empty() {
            updates.shortcut_open_settings = Some(self.shortcut_open_settings.clone());
        }
        if !self.shortcut_next_tab.trim().is_empty() {
            updates.shortcut_next_tab = Some(self.shortcut_next_tab.clone());
        }
        if !self.shortcut_prev_tab.trim().is_empty() {
            updates.shortcut_prev_tab = Some(self.shortcut_prev_tab.clone());
        }
        if !self.shortcut_quit.trim().is_empty() {
            updates.shortcut_quit = Some(self.shortcut_quit.clone());
        }
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
        SettingsCategory::Shortcuts => shortcuts::view(config, draft),
    }
}

pub fn input_row<'a>(label: &'a str, value: &'a str, field: SettingsField) -> Element<'a, Message> {
    column(vec![
        text(label).size(13).into(),
        text_input("", value)
            .on_input(move |next| Message::SettingsInputChanged(field, next))
            .padding([8, 10])
            .width(Length::Fill)
            .into(),
    ])
    .spacing(SPACING_SMALL)
    .width(Length::Fill)
    .into()
}

pub fn toggle_row<'a>(label: &'a str, value: bool) -> Element<'a, Message> {
    row![
        text(label).size(13),
        toggler(value)
            .on_toggle(Message::SettingsBlurToggled)
            .size(18)
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

pub fn section<'a>(title: &'a str, body: Element<'a, Message>) -> Element<'a, Message> {
    let palette = Palette::DARK;
    container(
        column(vec![text(title).size(15).into(), body])
            .spacing(SPACING_SMALL)
            .width(Length::Fill),
    )
    .padding(12)
    .width(Length::Fill)
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.28,
            ..palette.surface
        })),
        border: Border {
            radius: RADIUS_NORMAL.into(),
            width: 1.0,
            color: Color {
                a: 0.12,
                ..palette.text
            },
        },
        ..Default::default()
    })
    .into()
}

pub fn format_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}
