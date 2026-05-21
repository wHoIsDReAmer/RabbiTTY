use crate::config::{
    AppConfig, AppConfigUpdates, BellMode, CursorShape, SshAuthMethod, SshProfile, parse_hex_color,
};
use crate::gui::app::Message;
use crate::gui::components::accent_toggler_style;
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL};
use iced::widget::{button, column, container, row, rule, text, text_input, toggler};
use iced::{Alignment, Background, Border, Color, Element, Length};
use std::fmt;

pub mod appearance;
pub mod shortcuts;
pub mod ssh;
pub mod terminal;
pub mod theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFontOption {
    pub label: String,
    pub value: String,
    pub monospaced: bool,
}

impl fmt::Display for TerminalFontOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    AppearanceLanguage,
    TerminalFontSelection,
    TerminalFontSize,
    TerminalPaddingX,
    TerminalPaddingY,
    TerminalScrollback,
    TerminalScrollSpeed,
    ThemeColorScheme,
    ThemeForeground,
    ThemeBackground,
    ThemeCursor,
    ThemeBackgroundOpacity,
    #[allow(dead_code)]
    ThemeMacosBlurRadius,
    ShortcutNewTab,
    ShortcutCloseTab,
    ShortcutOpenSettings,
    ShortcutNextTab,
    ShortcutPrevTab,
    ShortcutQuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshProfileField {
    Name,
    Host,
    Port,
    User,
    AuthMethod,
    IdentityFile,
    Password,
    ProxyCommandEnabled,
    ProxyCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsCategory {
    Appearance,
    Terminal,
    Theme,
    Shortcuts,
    Ssh,
}

impl SettingsCategory {
    pub const ALL: [Self; 5] = [
        Self::Appearance,
        Self::Terminal,
        Self::Theme,
        Self::Shortcuts,
        Self::Ssh,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => crate::t!("settings.categories.appearance"),
            Self::Terminal => crate::t!("settings.categories.terminal"),
            Self::Theme => crate::t!("settings.categories.theme"),
            Self::Shortcuts => crate::t!("settings.categories.shortcuts"),
            Self::Ssh => crate::t!("settings.categories.ssh"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Appearance => "◫",
            Self::Terminal => "▣",
            Self::Theme => "◑",
            Self::Shortcuts => "⌘",
            Self::Ssh => "⇄",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SshProfileDraft {
    pub name: String,
    pub host: String,
    pub port: String,
    pub user: String,
    pub auth_method: SshAuthMethod,
    pub identity_file: String,
    pub password: String,
    pub proxy_command_enabled: bool,
    pub proxy_command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshProfileModalMode {
    Create,
    Edit(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshConnectionTestStatus {
    Idle,
    Testing,
    Success(String),
    Failure(String),
}

impl Default for SshProfileDraft {
    fn default() -> Self {
        Self::from_profile(&SshProfile::default())
    }
}

impl SshProfileDraft {
    pub fn from_profile(profile: &SshProfile) -> Self {
        Self {
            name: profile.name.clone(),
            host: profile.host.clone(),
            port: profile.port.to_string(),
            user: profile.user.clone(),
            auth_method: profile.auth_method,
            identity_file: profile.identity_file.clone().unwrap_or_default(),
            password: profile.password.clone().unwrap_or_default(),
            proxy_command_enabled: profile
                .proxy_command
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            proxy_command: profile.proxy_command.clone().unwrap_or_default(),
        }
    }

    pub fn to_profile(&self) -> Option<SshProfile> {
        let host = self.host.trim();
        if host.is_empty() {
            return None;
        }
        Some(SshProfile {
            name: self.name.trim().to_string(),
            host: host.to_string(),
            port: self.port.trim().parse().unwrap_or(22),
            user: self.user.trim().to_string(),
            auth_method: self.auth_method,
            identity_file: if matches!(self.auth_method, SshAuthMethod::KeyFile) {
                let v = self.identity_file.trim();
                if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                }
            } else {
                None
            },
            password: if matches!(self.auth_method, SshAuthMethod::Password) {
                let v = self.password.trim();
                if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                }
            } else {
                None
            },
            proxy_command: if self.proxy_command_enabled {
                let v = self.proxy_command.trim();
                if v.is_empty() {
                    None
                } else {
                    Some(v.to_string())
                }
            } else {
                None
            },
        })
    }

    fn is_blank(&self) -> bool {
        self.name.trim().is_empty()
            && self.host.trim().is_empty()
            && self.user.trim().is_empty()
            && self.identity_file.trim().is_empty()
            && self.password.trim().is_empty()
            && (!self.proxy_command_enabled || self.proxy_command.trim().is_empty())
            && self.port.trim().parse::<u16>().unwrap_or(22) == 22
    }
}

#[derive(Debug, Clone)]
pub struct SettingsDraft {
    pub language: String,
    pub terminal_font_selection: String,
    pub terminal_font_size: String,
    pub terminal_padding_x: String,
    pub terminal_padding_y: String,
    pub terminal_scrollback: String,
    pub terminal_scroll_speed: String,
    pub bracketed_paste: bool,
    pub multiline_paste_confirm: bool,
    pub cursor_shape: CursorShape,
    pub cursor_blink: bool,
    pub bell_mode: BellMode,
    pub color_scheme: String,
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub background_opacity: String,
    pub blur_enabled: bool,
    pub animations_enabled: bool,
    pub macos_blur_radius: String,
    pub shortcut_new_tab: String,
    pub shortcut_close_tab: String,
    pub shortcut_open_settings: String,
    pub shortcut_next_tab: String,
    pub shortcut_prev_tab: String,
    pub shortcut_quit: String,
    pub ssh_profiles: Vec<SshProfileDraft>,
    pub ssh_profiles_error: Option<String>,
    pub ssh_profile_modal_mode: Option<SshProfileModalMode>,
    pub ssh_profile_modal_draft: SshProfileDraft,
    pub ssh_profile_delete_pending: Option<usize>,
    pub ssh_connection_test_status: SshConnectionTestStatus,
}

impl SettingsDraft {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            language: config
                .ui
                .language
                .clone()
                .unwrap_or_else(|| "auto".to_string()),
            terminal_font_selection: config.terminal.font_selection.clone().unwrap_or_default(),
            terminal_font_size: format!("{:.1}", config.terminal.font_size),
            terminal_padding_x: format!("{:.1}", config.terminal.padding_x),
            terminal_padding_y: format!("{:.1}", config.terminal.padding_y),
            terminal_scrollback: config.terminal.scrollback_lines.to_string(),
            terminal_scroll_speed: format!("{:.1}", config.terminal.scroll_multiplier),
            bracketed_paste: config.terminal.bracketed_paste,
            multiline_paste_confirm: config.terminal.multiline_paste_confirm,
            cursor_shape: config.terminal.cursor_shape,
            cursor_blink: config.terminal.cursor_blink,
            bell_mode: config.terminal.bell_mode,
            color_scheme: config.theme.color_scheme.clone(),
            foreground: format_rgb(config.theme.foreground),
            background: format_rgb(config.theme.background),
            cursor: format_rgb(config.theme.cursor),
            background_opacity: format!("{:.2}", config.theme.background_opacity),
            blur_enabled: config.theme.blur_enabled,
            animations_enabled: config.ui.animations_enabled,
            macos_blur_radius: format!("{}", config.theme.macos_blur_radius),
            shortcut_new_tab: config.shortcuts.new_tab.clone(),
            shortcut_close_tab: config.shortcuts.close_tab.clone(),
            shortcut_open_settings: config.shortcuts.open_settings.clone(),
            shortcut_next_tab: config.shortcuts.next_tab.clone(),
            shortcut_prev_tab: config.shortcuts.prev_tab.clone(),
            shortcut_quit: config.shortcuts.quit.clone(),
            ssh_profiles: config
                .ssh_profiles
                .iter()
                .map(SshProfileDraft::from_profile)
                .collect(),
            ssh_profiles_error: None,
            ssh_profile_modal_mode: None,
            ssh_profile_modal_draft: SshProfileDraft::default(),
            ssh_profile_delete_pending: None,
            ssh_connection_test_status: SshConnectionTestStatus::Idle,
        }
    }

    #[cfg(test)]
    pub fn update_ssh_profile(&mut self, index: usize, field: SshProfileField, value: String) {
        self.ssh_profiles_error = None;
        if let Some(draft) = self.ssh_profiles.get_mut(index) {
            update_ssh_profile_draft(draft, field, value);
        }
    }

    #[cfg(test)]
    pub fn add_ssh_profile(&mut self) {
        self.ssh_profiles_error = None;
        self.ssh_profiles.push(SshProfileDraft::default());
    }

    pub fn request_delete_ssh_profile(&mut self, index: usize) {
        if index < self.ssh_profiles.len() {
            self.ssh_profiles_error = None;
            self.ssh_profile_delete_pending = Some(index);
        }
    }

    pub fn cancel_delete_ssh_profile(&mut self) {
        self.ssh_profile_delete_pending = None;
    }

    pub fn confirm_delete_ssh_profile(&mut self) -> Option<(String, String)> {
        let index = self.ssh_profile_delete_pending.take()?;
        self.ssh_profiles_error = None;
        if index < self.ssh_profiles.len() {
            let profile = self.ssh_profiles.remove(index);
            return Some((profile.host, profile.user));
        }
        None
    }

    pub fn open_create_ssh_profile_modal(&mut self) {
        self.ssh_profiles_error = None;
        self.ssh_profile_modal_mode = Some(SshProfileModalMode::Create);
        self.ssh_profile_modal_draft = SshProfileDraft::default();
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
    }

    pub fn open_edit_ssh_profile_modal(&mut self, index: usize) {
        if let Some(profile) = self.ssh_profiles.get(index) {
            self.ssh_profiles_error = None;
            self.ssh_profile_modal_mode = Some(SshProfileModalMode::Edit(index));
            let mut draft = profile.clone();
            if matches!(draft.auth_method, SshAuthMethod::Password)
                && draft.password.is_empty()
                && let Some(pw) = crate::keychain::get_password(&draft.host, &draft.user)
            {
                draft.password = pw;
            }
            self.ssh_profile_modal_draft = draft;
            self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        }
    }

    pub fn close_ssh_profile_modal(&mut self) {
        self.ssh_profile_modal_mode = None;
        self.ssh_profile_modal_draft = SshProfileDraft::default();
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
    }

    pub fn update_ssh_profile_modal(&mut self, field: SshProfileField, value: String) {
        self.ssh_profiles_error = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        update_ssh_profile_draft(&mut self.ssh_profile_modal_draft, field, value);
    }

    pub fn begin_ssh_connection_test(&mut self) -> Result<SshProfile, String> {
        let Some(profile) = self.ssh_profile_modal_draft.to_profile() else {
            let message = crate::t!("settings.ssh.status.host_required").to_string();
            self.ssh_connection_test_status = SshConnectionTestStatus::Failure(message.clone());
            return Err(message);
        };
        self.ssh_profiles_error = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Testing;
        Ok(profile)
    }

    pub fn finish_ssh_connection_test(&mut self, result: Result<(), String>) {
        self.ssh_connection_test_status = match result {
            Ok(()) => SshConnectionTestStatus::Success(
                crate::t!("settings.ssh.status.connection_successful").to_string(),
            ),
            Err(message) => SshConnectionTestStatus::Failure(message),
        };
    }

    pub fn save_ssh_profile_modal(&mut self) -> Result<Option<SshProfile>, String> {
        if self.ssh_profile_modal_mode.is_none() {
            return Ok(None);
        }
        let Some(profile) = self.ssh_profile_modal_draft.to_profile() else {
            let message = crate::t!("settings.ssh.status.host_required_save").to_string();
            self.ssh_profiles_error = Some(message.clone());
            return Err(message);
        };

        match self.ssh_profile_modal_mode {
            Some(SshProfileModalMode::Create) => {
                self.ssh_profiles.push(self.ssh_profile_modal_draft.clone());
            }
            Some(SshProfileModalMode::Edit(index)) => {
                if let Some(slot) = self.ssh_profiles.get_mut(index) {
                    *slot = self.ssh_profile_modal_draft.clone();
                }
            }
            None => {}
        }

        self.close_ssh_profile_modal();
        self.ssh_profiles_error = None;
        Ok(Some(profile))
    }

    pub fn apply_ssh_profiles_to(&mut self, profiles: &mut Vec<SshProfile>) -> Result<(), String> {
        let mut next = Vec::new();
        for (index, draft) in self.ssh_profiles.iter().enumerate() {
            let Some(profile) = draft.to_profile() else {
                if draft.is_blank() {
                    continue;
                }
                let message = format!("SSH profile {} needs a Host before saving.", index + 1);
                self.ssh_profiles_error = Some(message.clone());
                return Err(message);
            };
            next.push(profile);
        }
        *profiles = next;
        self.ssh_profiles_error = None;
        Ok(())
    }

    pub fn set_ssh_profiles_error(&mut self, message: impl Into<String>) {
        self.ssh_profiles_error = Some(message.into());
    }

    pub fn set_ssh_profiles_saved(&mut self) {
        self.ssh_profiles_error = Some(crate::t!("settings.ssh.status.profiles_saved").to_string());
    }

    pub fn update(&mut self, field: SettingsField, value: String) {
        match field {
            SettingsField::AppearanceLanguage => self.language = value,
            SettingsField::TerminalFontSelection => self.terminal_font_selection = value,
            SettingsField::TerminalFontSize => self.terminal_font_size = value,
            SettingsField::TerminalPaddingX => self.terminal_padding_x = value,
            SettingsField::TerminalPaddingY => self.terminal_padding_y = value,
            SettingsField::TerminalScrollback => self.terminal_scrollback = value,
            SettingsField::TerminalScrollSpeed => self.terminal_scroll_speed = value,
            SettingsField::ThemeColorScheme => {
                self.color_scheme = value.clone();
                if let Some(preset) = crate::terminal::theme::find_preset(&value) {
                    self.foreground = format_rgb(preset.fg);
                    self.background = format_rgb(preset.bg);
                    self.cursor = format_rgb(preset.cursor);
                }
            }
            SettingsField::ThemeForeground => self.foreground = value,
            SettingsField::ThemeBackground => self.background = value,
            SettingsField::ThemeCursor => self.cursor = value,
            SettingsField::ThemeBackgroundOpacity => self.background_opacity = value,
            SettingsField::ThemeMacosBlurRadius => self.macos_blur_radius = value,
            SettingsField::ShortcutNewTab => self.shortcut_new_tab = value,
            SettingsField::ShortcutCloseTab => self.shortcut_close_tab = value,
            SettingsField::ShortcutOpenSettings => self.shortcut_open_settings = value,
            SettingsField::ShortcutNextTab => self.shortcut_next_tab = value,
            SettingsField::ShortcutPrevTab => self.shortcut_prev_tab = value,
            SettingsField::ShortcutQuit => self.shortcut_quit = value,
        }
    }

    #[allow(dead_code)]
    pub fn to_updates(&self) -> AppConfigUpdates {
        let ansi_colors = crate::terminal::theme::find_preset(&self.color_scheme).map(|p| p.ansi);

        let mut updates = AppConfigUpdates {
            language: Some(self.language.clone()),
            animations_enabled: Some(self.animations_enabled),
            terminal_font_selection: Some(self.terminal_font_selection.clone()),
            terminal_font_size: parse_f32(&self.terminal_font_size),
            terminal_padding_x: parse_f32(&self.terminal_padding_x),
            terminal_padding_y: parse_f32(&self.terminal_padding_y),
            terminal_scrollback: self.terminal_scrollback.trim().parse::<usize>().ok(),
            terminal_scroll_multiplier: parse_f32(&self.terminal_scroll_speed),
            terminal_bracketed_paste: Some(self.bracketed_paste),
            terminal_multiline_paste_confirm: Some(self.multiline_paste_confirm),
            terminal_cursor_shape: Some(self.cursor_shape),
            terminal_cursor_blink: Some(self.cursor_blink),
            terminal_bell_mode: Some(self.bell_mode),
            color_scheme: Some(self.color_scheme.clone()),
            foreground: parse_hex_color(&self.foreground),
            background: parse_hex_color(&self.background),
            cursor: parse_hex_color(&self.cursor),
            ansi_colors,
            background_opacity: parse_f32(&self.background_opacity),
            blur_enabled: Some(self.blur_enabled),
            macos_blur_radius: self.macos_blur_radius.trim().parse::<i32>().ok(),
            ..Default::default()
        };

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

#[allow(dead_code)]
fn parse_f32(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok()
}

fn update_ssh_profile_draft(draft: &mut SshProfileDraft, field: SshProfileField, value: String) {
    match field {
        SshProfileField::Name => draft.name = value,
        SshProfileField::Host => draft.host = value,
        SshProfileField::Port => draft.port = value,
        SshProfileField::User => draft.user = value,
        SshProfileField::AuthMethod => {
            draft.auth_method = match value.as_str() {
                "key_file" => SshAuthMethod::KeyFile,
                "password" => SshAuthMethod::Password,
                _ => draft.auth_method,
            };
        }
        SshProfileField::IdentityFile => draft.identity_file = value,
        SshProfileField::Password => draft.password = value,
        SshProfileField::ProxyCommandEnabled => {
            draft.proxy_command_enabled = value == "true";
        }
        SshProfileField::ProxyCommand => draft.proxy_command = value,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn view_category<'a>(
    category: SettingsCategory,
    config: &'a AppConfig,
    draft: &'a SettingsDraft,
    font_combo_state: &'a iced::widget::combo_box::State<TerminalFontOption>,
    show_all_fonts: bool,
    all_font_options: &'a [TerminalFontOption],
    ssh_config_profiles: &'a [SshProfile],
    palette: Palette,
) -> Element<'a, Message> {
    let animations_enabled = config.ui.animations_enabled;
    match category {
        SettingsCategory::Appearance => {
            let selected_font = all_font_options
                .iter()
                .find(|o| o.value == draft.terminal_font_selection);
            appearance::view(
                config,
                draft,
                font_combo_state,
                show_all_fonts,
                selected_font,
                palette,
            )
        }
        SettingsCategory::Terminal => terminal::view(config, draft, palette),
        SettingsCategory::Theme => theme::view(config, draft, palette),
        SettingsCategory::Shortcuts => shortcuts::view(config, draft, palette),
        SettingsCategory::Ssh => ssh::view(draft, ssh_config_profiles, palette, animations_enabled),
    }
}

const LABEL_WIDTH: f32 = 160.0;

pub fn input_row<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
    palette: Palette,
) -> Element<'a, Message> {
    let commit_msg = Message::SettingsInputCommitted(field, value.to_owned());
    row![
        text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
        styled_text_input(
            value,
            move |next| Message::SettingsInputChanged(field, next),
            palette
        )
        .on_submit(commit_msg),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

pub fn input_row_with_suffix<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
    suffix: &'a str,
    palette: Palette,
) -> Element<'a, Message> {
    let commit_msg = Message::SettingsInputCommitted(field, value.to_owned());
    row![
        text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
        styled_text_input(
            value,
            move |next| Message::SettingsInputChanged(field, next),
            palette
        )
        .on_submit(commit_msg),
        text(suffix)
            .size(12)
            .color(palette.text_secondary)
            .width(Length::Shrink),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

#[allow(dead_code)]
pub fn color_input_row<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
    palette: Palette,
) -> Element<'a, Message> {
    let parsed = parse_hex_color(value);
    let dot_color = parsed
        .map(|rgb| Color::from_rgb8(rgb[0], rgb[1], rgb[2]))
        .unwrap_or(palette.error);
    let commit_msg = Message::SettingsInputCommitted(field, value.to_owned());

    row![
        text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
        container("")
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0))
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(dot_color)),
                border: Border {
                    radius: 9.0.into(),
                    width: 1.0,
                    color: Color {
                        a: 0.25,
                        ..palette.text
                    },
                },
                ..Default::default()
            }),
        styled_text_input(
            value,
            move |next| Message::SettingsInputChanged(field, next),
            palette
        )
        .on_submit(commit_msg),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

pub fn toggle_row<'a>(label: &'a str, value: bool, palette: Palette) -> Element<'a, Message> {
    row![
        text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
        toggler(value)
            .on_toggle(Message::SettingsBlurToggled)
            .size(18)
            .style(accent_toggler_style(palette))
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

/// A labeled segmented control: a fixed-width label followed by a row of
/// mutually-exclusive buttons. The selected segment is accent-styled, others
/// are dim. Each segment supplies its own pre-built `Message`.
pub fn segmented_control<'a>(
    label: &'a str,
    segments: Vec<(&'a str, Message, bool)>,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let buttons: Vec<Element<'a, Message>> = segments
        .into_iter()
        .map(|(segment_label, message, selected)| {
            segment_button(
                segment_label,
                message,
                selected,
                palette,
                animations_enabled,
            )
        })
        .collect();

    row![
        text(label).size(13).width(Length::Fixed(LABEL_WIDTH)),
        row(buttons).spacing(8),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn segment_button<'a>(
    label: &'a str,
    message: Message,
    selected: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let accent = palette.accent;
    let text_color = palette.text;
    let surface = palette.surface;

    // The visible background + border is painted by `hover_fade` behind the
    // button so it can cross-fade on hover; the button itself stays
    // transparent.
    let inner = button(
        text(label)
            .size(13)
            .color(if selected { accent } else { text_color }),
    )
    .on_press(message)
    .padding([6, 14])
    .style(move |_theme: &iced::Theme, _status| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: if selected { accent } else { text_color },
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Default::default(),
        snap: true,
    });

    let rest = if selected {
        crate::gui::components::HoverStyle {
            background: Color { a: 0.25, ..surface },
            border_color: accent,
            border_width: 1.5,
            radius: RADIUS_SMALL,
        }
    } else {
        crate::gui::components::HoverStyle {
            background: Color { a: 0.12, ..surface },
            border_color: Color {
                a: 0.15,
                ..text_color
            },
            border_width: 1.0,
            radius: RADIUS_SMALL,
        }
    };
    // Selected segments do not change on hover; non-selected ones brighten.
    let hover = if selected {
        rest
    } else {
        crate::gui::components::HoverStyle {
            background: Color { a: 0.2, ..surface },
            border_color: Color {
                a: 0.28,
                ..text_color
            },
            border_width: 1.0,
            radius: RADIUS_SMALL,
        }
    };

    crate::gui::components::hover_fade(inner, rest, hover, animations_enabled).into()
}

pub fn hint_text<'a>(msg: &'a str, palette: Palette) -> Element<'a, Message> {
    text(msg).size(11).color(palette.text_secondary).into()
}

#[allow(dead_code)]
pub fn divider<'a>(palette: Palette) -> Element<'a, Message> {
    container(
        rule::horizontal(1).style(move |_theme: &iced::Theme| rule::Style {
            color: Color {
                a: 0.10,
                ..palette.text
            },
            radius: 0.0.into(),
            fill_mode: rule::FillMode::Full,
            snap: false,
        }),
    )
    .padding([4, 0])
    .width(Length::Fill)
    .into()
}

pub fn section<'a>(
    title: &'a str,
    body: Element<'a, Message>,
    palette: Palette,
) -> Element<'a, Message> {
    container(
        column(vec![
            text(title).size(14).color(palette.text).into(),
            container("")
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.10,
                        ..palette.text
                    })),
                    ..Default::default()
                })
                .into(),
            body,
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill),
    )
    .padding([16, 16])
    .width(Length::Fill)
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.18,
            ..palette.surface
        })),
        border: Border {
            radius: RADIUS_NORMAL.into(),
            width: 1.0,
            color: Color {
                a: 0.08,
                ..palette.text
            },
        },
        ..Default::default()
    })
    .into()
}

fn styled_text_input<'a, F>(
    value: &'a str,
    on_input: F,
    palette: Palette,
) -> text_input::TextInput<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    text_input("", value)
        .on_input(on_input)
        .padding([6, 10])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme, status: text_input::Status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(Color {
                    a: 0.35,
                    ..palette.background
                }),
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 1.0,
                    color: if focused {
                        Color {
                            a: 0.5,
                            ..palette.accent
                        }
                    } else {
                        Color {
                            a: 0.12,
                            ..palette.text
                        }
                    },
                },
                icon: palette.text_secondary,
                placeholder: palette.text_secondary,
                value: palette.text,
                selection: Color {
                    a: 0.3,
                    ..palette.accent
                },
            }
        })
}

pub fn styled_text_input_small<'a, F>(
    value: &'a str,
    on_input: F,
    palette: Palette,
) -> text_input::TextInput<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    text_input("", value)
        .on_input(on_input)
        .padding([4, 8])
        .width(Length::Fixed(100.0))
        .style(move |_theme: &iced::Theme, status: text_input::Status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(Color {
                    a: 0.35,
                    ..palette.background
                }),
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 1.0,
                    color: if focused {
                        Color {
                            a: 0.5,
                            ..palette.accent
                        }
                    } else {
                        Color {
                            a: 0.12,
                            ..palette.text
                        }
                    },
                },
                icon: palette.text_secondary,
                placeholder: palette.text_secondary,
                value: palette.text,
                selection: Color {
                    a: 0.3,
                    ..palette.accent
                },
            }
        })
}

pub fn format_rgb(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SshAuthMethod, SshProfile};

    #[test]
    fn ssh_draft_roundtrip_with_password() {
        let profile = SshProfile {
            name: "prod".into(),
            host: "10.0.0.1".into(),
            port: 2222,
            user: "deploy".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: Some("~/.ssh/id_rsa".into()),
            password: Some("s3cret".into()),
            proxy_command: None,
        };

        let draft = SshProfileDraft::from_profile(&profile);
        assert_eq!(draft.name, "prod");
        assert_eq!(draft.host, "10.0.0.1");
        assert_eq!(draft.port, "2222");
        assert_eq!(draft.user, "deploy");
        assert_eq!(draft.auth_method, SshAuthMethod::Password);
        assert_eq!(draft.identity_file, "~/.ssh/id_rsa");
        assert_eq!(draft.password, "s3cret");

        let back = draft.to_profile().unwrap();
        assert_eq!(back.auth_method, SshAuthMethod::Password);
        assert!(back.identity_file.is_none());
        assert_eq!(back.password.as_deref(), Some("s3cret"));
        assert_eq!(back.port, 2222);
    }

    #[test]
    fn ssh_draft_key_file_auth_ignores_password() {
        let draft = SshProfileDraft {
            name: "test".into(),
            host: "host".into(),
            port: "22".into(),
            user: "me".into(),
            auth_method: SshAuthMethod::KeyFile,
            identity_file: "~/.ssh/id_ed25519".into(),
            password: "saved-password".into(),
            proxy_command_enabled: true,
            proxy_command: "  cloudflared access ssh --hostname %h  ".into(),
        };

        let profile = draft.to_profile().unwrap();

        assert_eq!(profile.auth_method, SshAuthMethod::KeyFile);
        assert_eq!(profile.identity_file.as_deref(), Some("~/.ssh/id_ed25519"));
        assert!(profile.password.is_none());
        assert_eq!(
            profile.proxy_command.as_deref(),
            Some("cloudflared access ssh --hostname %h")
        );
    }

    #[test]
    fn ssh_draft_proxy_command_requires_enabled_flag() {
        let mut draft = SshProfileDraft {
            name: "test".into(),
            host: "host".into(),
            port: "22".into(),
            user: "me".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "secret".into(),
            proxy_command_enabled: false,
            proxy_command: "cloudflared access ssh --hostname %h".into(),
        };

        let disabled = draft.to_profile().unwrap();
        assert!(disabled.proxy_command.is_none());

        draft.proxy_command_enabled = true;
        let enabled = draft.to_profile().unwrap();
        assert_eq!(
            enabled.proxy_command.as_deref(),
            Some("cloudflared access ssh --hostname %h")
        );
    }

    #[test]
    fn ssh_draft_from_profile_enables_proxy_command_when_present() {
        let profile = SshProfile {
            name: "proxy".into(),
            host: "proxy.example.com".into(),
            port: 22,
            user: "deploy".into(),
            auth_method: SshAuthMethod::KeyFile,
            identity_file: Some("~/.ssh/id_ed25519".into()),
            password: None,
            proxy_command: Some("cloudflared access ssh --hostname %h".into()),
        };

        let draft = SshProfileDraft::from_profile(&profile);

        assert!(draft.proxy_command_enabled);
        assert_eq!(draft.proxy_command, "cloudflared access ssh --hostname %h");
    }

    #[test]
    fn ssh_draft_empty_password_becomes_none() {
        let draft = SshProfileDraft {
            name: "test".into(),
            host: "host".into(),
            port: "22".into(),
            user: "".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "  ".into(),
            proxy_command_enabled: false,
            proxy_command: "".into(),
        };
        let profile = draft.to_profile().unwrap();
        assert!(profile.password.is_none());
        assert!(profile.identity_file.is_none());
    }

    #[test]
    fn ssh_draft_empty_host_returns_none() {
        let draft = SshProfileDraft {
            name: "test".into(),
            host: "  ".into(),
            port: "22".into(),
            user: "".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "pass".into(),
            proxy_command_enabled: false,
            proxy_command: "".into(),
        };
        assert!(draft.to_profile().is_none());
    }

    #[test]
    fn update_ssh_profile_password_field() {
        let config = crate::config::AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "srv".into(),
                host: "h".into(),
                port: 22,
                user: "u".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: None,
                proxy_command: None,
            }],
            ..Default::default()
        };
        let mut draft = SettingsDraft::from_config(&config);
        assert_eq!(draft.ssh_profiles[0].password, "");

        draft.update_ssh_profile(0, SshProfileField::Password, "newpass".into());
        assert_eq!(draft.ssh_profiles[0].password, "newpass");
    }

    #[test]
    fn ssh_profile_modal_create_appends_profile() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_ssh_profile_modal();
        draft.update_ssh_profile_modal(SshProfileField::Name, "prod".into());
        draft.update_ssh_profile_modal(SshProfileField::Host, "10.0.0.1".into());
        draft.update_ssh_profile_modal(SshProfileField::User, "deploy".into());
        draft.save_ssh_profile_modal().unwrap();

        assert_eq!(draft.ssh_profiles.len(), 1);
        assert_eq!(draft.ssh_profiles[0].name, "prod");
        assert_eq!(draft.ssh_profiles[0].host, "10.0.0.1");
        assert!(draft.ssh_profile_modal_mode.is_none());
    }

    #[test]
    fn ssh_profile_modal_edit_replaces_selected_profile() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "old".into(),
                host: "old.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::KeyFile,
                identity_file: Some("~/.ssh/id_ed25519".into()),
                password: None,
                proxy_command: None,
            }],
            ..Default::default()
        });

        draft.open_edit_ssh_profile_modal(0);
        draft.update_ssh_profile_modal(SshProfileField::Name, "new".into());
        draft.update_ssh_profile_modal(SshProfileField::Host, "new.example.com".into());
        draft.save_ssh_profile_modal().unwrap();

        assert_eq!(draft.ssh_profiles.len(), 1);
        assert_eq!(draft.ssh_profiles[0].name, "new");
        assert_eq!(draft.ssh_profiles[0].host, "new.example.com");
        assert_eq!(draft.ssh_profiles[0].identity_file, "~/.ssh/id_ed25519");
    }

    #[test]
    fn ssh_profile_modal_cancel_leaves_profiles_unchanged() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "prod".into(),
                host: "prod.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret".into()),
                proxy_command: None,
            }],
            ..Default::default()
        });

        draft.open_edit_ssh_profile_modal(0);
        draft.update_ssh_profile_modal(SshProfileField::Host, "changed.example.com".into());
        draft.close_ssh_profile_modal();

        assert_eq!(draft.ssh_profiles[0].host, "prod.example.com");
        assert!(draft.ssh_profile_modal_mode.is_none());
    }

    #[test]
    fn ssh_profile_modal_save_requires_host() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_ssh_profile_modal();
        draft.update_ssh_profile_modal(SshProfileField::Name, "missing-host".into());
        let err = draft.save_ssh_profile_modal().unwrap_err();

        assert_eq!(err, crate::t!("settings.ssh.status.host_required_save"));
        assert!(draft.ssh_profiles.is_empty());
        assert!(draft.ssh_profile_modal_mode.is_some());
    }

    #[test]
    fn ssh_profile_delete_requires_confirmation() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: vec![
                SshProfile {
                    name: "prod".into(),
                    host: "prod.example.com".into(),
                    port: 22,
                    user: "deploy".into(),
                    auth_method: SshAuthMethod::Password,
                    identity_file: None,
                    password: Some("secret".into()),
                    proxy_command: None,
                },
                SshProfile {
                    name: "stage".into(),
                    host: "stage.example.com".into(),
                    port: 22,
                    user: "deploy".into(),
                    auth_method: SshAuthMethod::KeyFile,
                    identity_file: Some("~/.ssh/id_ed25519".into()),
                    password: None,
                    proxy_command: None,
                },
            ],
            ..Default::default()
        });

        draft.request_delete_ssh_profile(0);

        assert_eq!(draft.ssh_profile_delete_pending, Some(0));
        assert_eq!(draft.ssh_profiles.len(), 2);

        let removed = draft.confirm_delete_ssh_profile();

        assert_eq!(
            removed
                .as_ref()
                .map(|(host, user)| (host.as_str(), user.as_str())),
            Some(("prod.example.com", "deploy"))
        );
        assert_eq!(draft.ssh_profiles.len(), 1);
        assert_eq!(draft.ssh_profiles[0].host, "stage.example.com");
        assert!(draft.ssh_profile_delete_pending.is_none());
    }

    #[test]
    fn ssh_profile_delete_cancel_leaves_profile_unchanged() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: vec![SshProfile {
                name: "prod".into(),
                host: "prod.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret".into()),
                proxy_command: None,
            }],
            ..Default::default()
        });

        draft.request_delete_ssh_profile(0);
        draft.cancel_delete_ssh_profile();

        assert_eq!(draft.ssh_profiles.len(), 1);
        assert_eq!(draft.ssh_profiles[0].host, "prod.example.com");
        assert!(draft.ssh_profile_delete_pending.is_none());
    }

    #[test]
    fn ssh_connection_test_requires_host() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_ssh_profile_modal();
        let result = draft.begin_ssh_connection_test();

        assert!(result.is_err());
        assert_eq!(
            draft.ssh_connection_test_status,
            SshConnectionTestStatus::Failure(crate::t!("settings.ssh.status.host_required").into())
        );
    }

    #[test]
    fn ssh_connection_test_tracks_testing_and_result() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_ssh_profile_modal();
        draft.update_ssh_profile_modal(SshProfileField::Host, "example.com".into());
        let profile = draft.begin_ssh_connection_test().unwrap();

        assert_eq!(profile.host, "example.com");
        assert_eq!(
            draft.ssh_connection_test_status,
            SshConnectionTestStatus::Testing
        );

        draft.finish_ssh_connection_test(Err("Authentication failed".into()));
        assert_eq!(
            draft.ssh_connection_test_status,
            SshConnectionTestStatus::Failure("Authentication failed".into())
        );

        draft.finish_ssh_connection_test(Ok(()));
        assert_eq!(
            draft.ssh_connection_test_status,
            SshConnectionTestStatus::Success(
                crate::t!("settings.ssh.status.connection_successful").into()
            )
        );
    }

    #[test]
    fn apply_ssh_profiles_skips_blank_new_profile_without_clearing_existing_profiles() {
        let existing = SshProfile {
            name: "existing".into(),
            host: "existing.host".into(),
            port: 22,
            user: "u".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        let mut profiles = vec![existing.clone()];
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: profiles.clone(),
            ..Default::default()
        });
        draft.add_ssh_profile();

        let result = draft.apply_ssh_profiles_to(&mut profiles);

        assert!(result.is_ok());
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].host, "existing.host");
        assert!(draft.ssh_profiles_error.is_none());
    }

    #[test]
    fn apply_ssh_profiles_rejects_partial_profile_without_host() {
        let existing = SshProfile {
            name: "existing".into(),
            host: "existing.host".into(),
            port: 22,
            user: "u".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        let mut profiles = vec![existing.clone()];
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: profiles.clone(),
            ..Default::default()
        });
        draft.add_ssh_profile();
        draft.update_ssh_profile(1, SshProfileField::Name, "partial".into());

        let result = draft.apply_ssh_profiles_to(&mut profiles);

        assert!(result.is_err());
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].host, "existing.host");
        assert_eq!(
            draft.ssh_profiles_error.as_deref(),
            Some("SSH profile 2 needs a Host before saving.")
        );
    }

    #[test]
    fn apply_ssh_profiles_saves_valid_new_profile_when_blank_card_exists() {
        let existing = SshProfile {
            name: "existing".into(),
            host: "existing.host".into(),
            port: 22,
            user: "u".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        };
        let mut profiles = vec![existing.clone()];
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            ssh_profiles: profiles.clone(),
            ..Default::default()
        });
        draft.add_ssh_profile();
        draft.update_ssh_profile(1, SshProfileField::Name, "new".into());
        draft.update_ssh_profile(1, SshProfileField::Host, "new.host".into());
        draft.update_ssh_profile(1, SshProfileField::User, "new-user".into());
        draft.add_ssh_profile();

        let result = draft.apply_ssh_profiles_to(&mut profiles);

        assert!(result.is_ok());
        assert_eq!(profiles.len(), 2);
        assert_eq!(profiles[0].host, "existing.host");
        assert_eq!(profiles[1].name, "new");
        assert_eq!(profiles[1].host, "new.host");
        assert_eq!(profiles[1].user, "new-user");
    }
}
