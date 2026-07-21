use crate::config::{
    AppConfig, AppConfigUpdates, BellMode, CursorShape, RightClickAction, SshAuthMethod,
    SshProfile, TabBarPosition, parse_hex_color,
};
use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::accent_toggler_style;
use crate::gui::tab::{Profile, ProfileKind};
use crate::gui::theme::{Palette, RADIUS_SMALL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{Space, button, column, container, row, rule, text, text_input, toggler};
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
pub enum ProfileField {
    Name,
    Icon,
    Program,
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
pub enum ProfileDraftKind {
    Local,
    Ssh,
}

impl ProfileDraftKind {
    pub const ALL: [Self; 2] = [Self::Ssh, Self::Local];
}

impl fmt::Display for ProfileDraftKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ssh => crate::t!("settings.ssh.type_ssh"),
            Self::Local => crate::t!("settings.ssh.type_local"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileModalTab {
    #[default]
    Connection,
    Advanced,
}

impl ProfileModalTab {
    pub const ALL: [Self; 2] = [Self::Connection, Self::Advanced];

    pub fn label(self) -> &'static str {
        match self {
            Self::Connection => crate::t!("settings.ssh.tab_connection"),
            Self::Advanced => crate::t!("settings.ssh.tab_advanced"),
        }
    }
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
pub struct ProfileDraft {
    pub kind: ProfileDraftKind,
    pub name: String,
    pub icon: String,
    pub program: String,
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
pub enum ProfileModalMode {
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

impl Default for ProfileDraft {
    fn default() -> Self {
        let mut draft = Self::from_ssh_fields(&SshProfile::default());
        draft.kind = ProfileDraftKind::Local;
        draft
    }
}

impl ProfileDraft {
    fn from_ssh_fields(profile: &SshProfile) -> Self {
        Self {
            kind: ProfileDraftKind::Ssh,
            name: profile.name.clone(),
            icon: String::new(),
            program: String::new(),
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

    pub fn from_profile(profile: &Profile) -> Self {
        let icon = profile.icon.clone().unwrap_or_default();
        match &profile.kind {
            ProfileKind::Local { program, .. } => Self {
                kind: ProfileDraftKind::Local,
                name: profile.name.clone(),
                icon,
                program: program.clone().unwrap_or_default(),
                ..Self::default()
            },
            ProfileKind::Ssh(ssh) => {
                let mut draft = Self::from_ssh_fields(ssh);
                draft.icon = icon;
                draft
            }
        }
    }

    fn icon_option(&self) -> Option<String> {
        let v = self.icon.trim();
        if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        }
    }

    pub fn to_ssh_profile(&self) -> Option<SshProfile> {
        if !matches!(self.kind, ProfileDraftKind::Ssh) {
            return None;
        }
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

    pub fn to_profile(&self) -> Option<Profile> {
        match self.kind {
            ProfileDraftKind::Local => {
                let program = self.program.trim();
                let (program, args) = if program.is_empty() {
                    (None, Vec::new())
                } else {
                    (Some(program.to_string()), vec!["-l".to_string()])
                };
                Some(Profile {
                    name: self.name.trim().to_string(),
                    icon: self.icon_option(),
                    kind: ProfileKind::Local { program, args },
                })
            }
            ProfileDraftKind::Ssh => {
                let ssh = self.to_ssh_profile()?;
                let mut profile = Profile::ssh(ssh);
                profile.icon = self.icon_option();
                Some(profile)
            }
        }
    }

    fn is_blank(&self) -> bool {
        match self.kind {
            ProfileDraftKind::Local => {
                self.name.trim().is_empty()
                    && self.program.trim().is_empty()
                    && self.icon.trim().is_empty()
            }
            ProfileDraftKind::Ssh => {
                self.name.trim().is_empty()
                    && self.host.trim().is_empty()
                    && self.user.trim().is_empty()
                    && self.identity_file.trim().is_empty()
                    && self.password.trim().is_empty()
                    && (!self.proxy_command_enabled || self.proxy_command.trim().is_empty())
                    && self.port.trim().parse::<u16>().unwrap_or(22) == 22
            }
        }
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
    pub bold_is_bright: bool,
    pub bell_mode: BellMode,
    pub right_click_action: RightClickAction,
    pub color_scheme: String,
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub background_opacity: String,
    pub blur_enabled: bool,
    pub animations_enabled: bool,
    pub tab_bar_position: TabBarPosition,
    pub macos_blur_radius: String,
    pub shortcut_new_tab: String,
    pub shortcut_close_tab: String,
    pub shortcut_open_settings: String,
    pub shortcut_next_tab: String,
    pub shortcut_prev_tab: String,
    pub shortcut_quit: String,
    pub profiles: Vec<ProfileDraft>,
    pub profiles_error: Option<String>,
    pub profile_modal_mode: Option<ProfileModalMode>,
    pub profile_modal_draft: ProfileDraft,
    pub profile_delete_pending: Option<usize>,
    pub profile_modal_tab: ProfileModalTab,
    pub profile_modal_base: Option<usize>,
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
            bold_is_bright: config.terminal.bold_is_bright,
            bell_mode: config.terminal.bell_mode,
            right_click_action: config.terminal.right_click_action,
            color_scheme: config.theme.color_scheme.clone(),
            foreground: format_rgb(config.theme.foreground),
            background: format_rgb(config.theme.background),
            cursor: format_rgb(config.theme.cursor),
            background_opacity: format!("{:.2}", config.theme.background_opacity),
            blur_enabled: config.theme.blur_enabled,
            animations_enabled: config.ui.animations_enabled,
            tab_bar_position: config.ui.tab_bar_position,
            macos_blur_radius: format!("{}", config.theme.macos_blur_radius),
            shortcut_new_tab: config.shortcuts.new_tab.clone(),
            shortcut_close_tab: config.shortcuts.close_tab.clone(),
            shortcut_open_settings: config.shortcuts.open_settings.clone(),
            shortcut_next_tab: config.shortcuts.next_tab.clone(),
            shortcut_prev_tab: config.shortcuts.prev_tab.clone(),
            shortcut_quit: config.shortcuts.quit.clone(),
            profiles: config
                .profiles
                .iter()
                .map(ProfileDraft::from_profile)
                .collect(),
            profiles_error: None,
            profile_modal_mode: None,
            profile_modal_draft: ProfileDraft::default(),
            profile_delete_pending: None,
            profile_modal_tab: ProfileModalTab::default(),
            profile_modal_base: None,
            ssh_connection_test_status: SshConnectionTestStatus::Idle,
        }
    }

    #[cfg(test)]
    pub fn update_profile(&mut self, index: usize, field: ProfileField, value: String) {
        self.profiles_error = None;
        if let Some(draft) = self.profiles.get_mut(index) {
            update_profile_draft(draft, field, value);
        }
    }

    #[cfg(test)]
    pub fn add_profile(&mut self) {
        self.profiles_error = None;
        self.profiles.push(ProfileDraft::default());
    }

    pub fn request_delete_profile(&mut self, index: usize) {
        if index < self.profiles.len() {
            self.profiles_error = None;
            self.profile_delete_pending = Some(index);
        }
    }

    pub fn cancel_delete_profile(&mut self) {
        self.profile_delete_pending = None;
    }

    pub fn confirm_delete_profile(&mut self) -> Option<ProfileDraft> {
        let index = self.profile_delete_pending.take()?;
        self.profiles_error = None;
        if index < self.profiles.len() {
            return Some(self.profiles.remove(index));
        }
        None
    }

    pub fn open_create_profile_modal(&mut self) {
        self.profiles_error = None;
        self.profile_modal_mode = Some(ProfileModalMode::Create);
        self.profile_modal_draft = ProfileDraft::default();
        self.profile_modal_tab = ProfileModalTab::default();
        self.profile_modal_base = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
    }

    pub fn open_edit_profile_modal(&mut self, index: usize) {
        if let Some(profile) = self.profiles.get(index) {
            self.profiles_error = None;
            self.profile_modal_mode = Some(ProfileModalMode::Edit(index));
            let mut draft = profile.clone();
            if matches!(draft.kind, ProfileDraftKind::Ssh)
                && matches!(draft.auth_method, SshAuthMethod::Password)
                && draft.password.is_empty()
                && let Some(pw) = crate::keychain::get_password(&draft.host, &draft.user)
            {
                draft.password = pw;
            }
            self.profile_modal_draft = draft;
            self.profile_modal_tab = ProfileModalTab::default();
            self.profile_modal_base = None;
            self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        }
    }

    pub fn close_profile_modal(&mut self) {
        self.profile_modal_mode = None;
        self.profile_modal_draft = ProfileDraft::default();
        self.profile_modal_tab = ProfileModalTab::default();
        self.profile_modal_base = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
    }

    pub fn update_profile_modal(&mut self, field: ProfileField, value: String) {
        self.profiles_error = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        update_profile_draft(&mut self.profile_modal_draft, field, value);
    }

    pub fn set_profile_modal_kind(&mut self, kind: ProfileDraftKind) {
        self.profiles_error = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        self.profile_modal_draft.kind = kind;
        self.profile_modal_tab = ProfileModalTab::default();
    }

    pub fn set_profile_modal_tab(&mut self, tab: ProfileModalTab) {
        self.profile_modal_tab = tab;
    }

    pub fn apply_profile_modal_base(&mut self, index: Option<usize>, base: Option<&SshProfile>) {
        self.profiles_error = None;
        self.ssh_connection_test_status = SshConnectionTestStatus::Idle;
        self.profile_modal_tab = ProfileModalTab::default();
        self.profile_modal_base = index;

        let icon = self.profile_modal_draft.icon.clone();
        let mut draft = match base {
            Some(ssh) => ProfileDraft::from_ssh_fields(ssh),
            None => ProfileDraft {
                kind: ProfileDraftKind::Ssh,
                ..ProfileDraft::default()
            },
        };
        draft.icon = icon;
        self.profile_modal_draft = draft;
    }

    pub fn begin_ssh_connection_test(&mut self) -> Result<SshProfile, String> {
        let Some(profile) = self.profile_modal_draft.to_ssh_profile() else {
            let message = crate::t!("settings.ssh.status.host_required").to_string();
            self.ssh_connection_test_status = SshConnectionTestStatus::Failure(message.clone());
            return Err(message);
        };
        self.profiles_error = None;
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

    pub fn save_profile_modal(&mut self) -> Result<Option<Profile>, String> {
        if self.profile_modal_mode.is_none() {
            return Ok(None);
        }
        let Some(profile) = self.profile_modal_draft.to_profile() else {
            let message = crate::t!("settings.ssh.status.host_required_save").to_string();
            self.profiles_error = Some(message.clone());
            return Err(message);
        };

        match self.profile_modal_mode {
            Some(ProfileModalMode::Create) => {
                self.profiles.push(self.profile_modal_draft.clone());
            }
            Some(ProfileModalMode::Edit(index)) => {
                if let Some(slot) = self.profiles.get_mut(index) {
                    *slot = self.profile_modal_draft.clone();
                }
            }
            None => {}
        }

        self.close_profile_modal();
        self.profiles_error = None;
        Ok(Some(profile))
    }

    pub fn collect_profiles(&self) -> Vec<Profile> {
        self.profiles
            .iter()
            .filter(|draft| !draft.is_blank())
            .filter_map(|draft| draft.to_profile())
            .collect()
    }

    pub fn set_profiles_error(&mut self, message: impl Into<String>) {
        self.profiles_error = Some(message.into());
    }

    pub fn set_profiles_saved(&mut self) {
        self.profiles_error = Some(crate::t!("settings.ssh.status.profiles_saved").to_string());
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
            tab_bar_position: Some(self.tab_bar_position),
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
            terminal_bold_is_bright: Some(self.bold_is_bright),
            terminal_bell_mode: Some(self.bell_mode),
            terminal_right_click_action: Some(self.right_click_action),
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

fn update_profile_draft(draft: &mut ProfileDraft, field: ProfileField, value: String) {
    match field {
        ProfileField::Name => draft.name = value,
        ProfileField::Icon => draft.icon = value,
        ProfileField::Program => draft.program = value,
        ProfileField::Host => draft.host = value,
        ProfileField::Port => draft.port = value,
        ProfileField::User => draft.user = value,
        ProfileField::AuthMethod => {
            draft.auth_method = match value.as_str() {
                "key_file" => SshAuthMethod::KeyFile,
                "password" => SshAuthMethod::Password,
                _ => draft.auth_method,
            };
        }
        ProfileField::IdentityFile => draft.identity_file = value,
        ProfileField::Password => draft.password = value,
        ProfileField::ProxyCommandEnabled => {
            draft.proxy_command_enabled = value == "true";
        }
        ProfileField::ProxyCommand => draft.proxy_command = value,
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
        SettingsCategory::Ssh => ssh::view(draft, palette, animations_enabled),
    }
}

const NUMERIC_INPUT_WIDTH: f32 = 110.0;
pub const SECTION_SPACING: f32 = 40.0;
pub const ROW_SPACING: f32 = 22.0;
const TEXT_INPUT_WIDTH: f32 = 260.0;

pub fn input_row<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
    palette: Palette,
) -> Element<'a, Message> {
    let commit_msg = Message::Settings(SettingsMessage::InputCommitted(field, value.to_owned()));
    setting_row(
        label,
        styled_text_input(
            value,
            move |next| Message::Settings(SettingsMessage::InputChanged(field, next)),
            palette,
        )
        .width(Length::Fixed(TEXT_INPUT_WIDTH))
        .on_submit(commit_msg),
        palette,
    )
}

pub fn input_row_with_suffix<'a>(
    label: &'a str,
    value: &'a str,
    field: SettingsField,
    suffix: &'a str,
    palette: Palette,
) -> Element<'a, Message> {
    let commit_msg = Message::Settings(SettingsMessage::InputCommitted(field, value.to_owned()));
    setting_row(
        label,
        row![
            styled_text_input(
                value,
                move |next| Message::Settings(SettingsMessage::InputChanged(field, next)),
                palette,
            )
            .width(Length::Fixed(NUMERIC_INPUT_WIDTH))
            .on_submit(commit_msg),
            text(suffix)
                .size(12)
                .color(palette.text_secondary)
                .width(Length::Fixed(20.0)),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_SMALL),
        palette,
    )
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
    let commit_msg = Message::Settings(SettingsMessage::InputCommitted(field, value.to_owned()));

    setting_row(
        label,
        row![
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
                move |next| Message::Settings(SettingsMessage::InputChanged(field, next)),
                palette,
            )
            .width(Length::Fixed(NUMERIC_INPUT_WIDTH))
            .on_submit(commit_msg),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_NORMAL),
        palette,
    )
}

pub fn toggle_row<'a>(label: &'a str, value: bool, palette: Palette) -> Element<'a, Message> {
    setting_row(
        label,
        toggler(value)
            .on_toggle(|a0| Message::Settings(SettingsMessage::BlurToggled(a0)))
            .size(18)
            .style(accent_toggler_style(palette)),
        palette,
    )
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

    setting_row(label, row(buttons).spacing(2), palette)
}

fn segment_button<'a>(
    label: &'a str,
    message: Message,
    selected: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let accent = palette.accent;
    let on_accent = palette.background;
    let text_color = palette.text;
    let surface = palette.surface;

    // The visible background + border is painted by `hover_fade` behind the
    // button so it can cross-fade on hover; the button itself stays
    // transparent.
    let inner = button(
        text(label)
            .size(13)
            .color(if selected { on_accent } else { text_color }),
    )
    .on_press(message)
    .padding([9, 16])
    .style(move |_theme: &iced::Theme, _status| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: if selected { on_accent } else { text_color },
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
            background: accent,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: RADIUS_SMALL,
        }
    } else {
        crate::gui::components::HoverStyle {
            background: Color { a: 0.5, ..surface },
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: RADIUS_SMALL,
        }
    };
    // Selected segments do not change on hover; non-selected ones brighten.
    let hover = if selected {
        rest
    } else {
        crate::gui::components::HoverStyle {
            background: Color { a: 0.85, ..surface },
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
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
    column(vec![
        text(title).size(22).color(palette.text).into(),
        column(vec![body]).spacing(SPACING_LARGE).into(),
    ])
    .spacing(SPACING_LARGE)
    .width(Length::Fill)
    .into()
}

/// A settings row: label on the left, control pinned to the right edge.
pub fn setting_row<'a>(
    label: &'a str,
    control: impl Into<Element<'a, Message>>,
    palette: Palette,
) -> Element<'a, Message> {
    row![
        text(label).size(14).color(palette.text),
        Space::new().width(Length::Fill),
        control.into(),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
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
        .padding([9, 12])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme, status: text_input::Status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(Color {
                    a: 0.55,
                    ..palette.background
                }),
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 1.0,
                    color: if focused {
                        palette.accent
                    } else {
                        Color::TRANSPARENT
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
        .padding([9, 12])
        .width(Length::Fixed(100.0))
        .style(move |_theme: &iced::Theme, status: text_input::Status| {
            let focused = matches!(status, text_input::Status::Focused { .. });
            text_input::Style {
                background: Background::Color(Color {
                    a: 0.55,
                    ..palette.background
                }),
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 1.0,
                    color: if focused {
                        palette.accent
                    } else {
                        Color::TRANSPARENT
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

    fn ssh_draft(profile: &SshProfile) -> ProfileDraft {
        ProfileDraft::from_profile(&Profile::ssh(profile.clone()))
    }

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

        let draft = ssh_draft(&profile);
        assert_eq!(draft.kind, ProfileDraftKind::Ssh);
        assert_eq!(draft.name, "prod");
        assert_eq!(draft.host, "10.0.0.1");
        assert_eq!(draft.port, "2222");
        assert_eq!(draft.user, "deploy");
        assert_eq!(draft.auth_method, SshAuthMethod::Password);
        assert_eq!(draft.identity_file, "~/.ssh/id_rsa");
        assert_eq!(draft.password, "s3cret");

        let back = draft.to_ssh_profile().unwrap();
        assert_eq!(back.auth_method, SshAuthMethod::Password);
        assert!(back.identity_file.is_none());
        assert_eq!(back.password.as_deref(), Some("s3cret"));
        assert_eq!(back.port, 2222);
    }

    #[test]
    fn ssh_draft_key_file_auth_ignores_password() {
        let draft = ProfileDraft {
            kind: ProfileDraftKind::Ssh,
            name: "test".into(),
            icon: String::new(),
            program: String::new(),
            host: "host".into(),
            port: "22".into(),
            user: "me".into(),
            auth_method: SshAuthMethod::KeyFile,
            identity_file: "~/.ssh/id_ed25519".into(),
            password: "saved-password".into(),
            proxy_command_enabled: true,
            proxy_command: "  cloudflared access ssh --hostname %h  ".into(),
        };

        let profile = draft.to_ssh_profile().unwrap();

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
        let mut draft = ProfileDraft {
            kind: ProfileDraftKind::Ssh,
            name: "test".into(),
            icon: String::new(),
            program: String::new(),
            host: "host".into(),
            port: "22".into(),
            user: "me".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "secret".into(),
            proxy_command_enabled: false,
            proxy_command: "cloudflared access ssh --hostname %h".into(),
        };

        let disabled = draft.to_ssh_profile().unwrap();
        assert!(disabled.proxy_command.is_none());

        draft.proxy_command_enabled = true;
        let enabled = draft.to_ssh_profile().unwrap();
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

        let draft = ssh_draft(&profile);

        assert!(draft.proxy_command_enabled);
        assert_eq!(draft.proxy_command, "cloudflared access ssh --hostname %h");
    }

    #[test]
    fn ssh_draft_empty_password_becomes_none() {
        let draft = ProfileDraft {
            kind: ProfileDraftKind::Ssh,
            name: "test".into(),
            icon: String::new(),
            program: String::new(),
            host: "host".into(),
            port: "22".into(),
            user: "".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "  ".into(),
            proxy_command_enabled: false,
            proxy_command: "".into(),
        };
        let profile = draft.to_ssh_profile().unwrap();
        assert!(profile.password.is_none());
        assert!(profile.identity_file.is_none());
    }

    #[test]
    fn ssh_draft_empty_host_returns_none() {
        let draft = ProfileDraft {
            kind: ProfileDraftKind::Ssh,
            name: "test".into(),
            icon: String::new(),
            program: String::new(),
            host: "  ".into(),
            port: "22".into(),
            user: "".into(),
            auth_method: SshAuthMethod::Password,
            identity_file: "".into(),
            password: "pass".into(),
            proxy_command_enabled: false,
            proxy_command: "".into(),
        };
        assert!(draft.to_ssh_profile().is_none());
        assert!(draft.to_profile().is_none());
    }

    #[test]
    fn local_draft_roundtrips_program_and_icon() {
        let profile = Profile {
            name: "My fish".into(),
            icon: Some("🐟".into()),
            kind: ProfileKind::Local {
                program: Some("/opt/bin/fish".into()),
                args: vec!["-l".into()],
            },
        };
        let draft = ProfileDraft::from_profile(&profile);
        assert_eq!(draft.kind, ProfileDraftKind::Local);
        assert_eq!(draft.name, "My fish");
        assert_eq!(draft.icon, "🐟");
        assert_eq!(draft.program, "/opt/bin/fish");

        let back = draft.to_profile().unwrap();
        assert_eq!(back.name, "My fish");
        assert_eq!(back.icon.as_deref(), Some("🐟"));
        assert!(matches!(
            back.kind,
            ProfileKind::Local { program: Some(p), args } if p == "/opt/bin/fish" && args == vec!["-l".to_string()]
        ));
    }

    #[test]
    fn local_draft_without_program_has_empty_args() {
        let draft = ProfileDraft::default();
        let profile = draft.to_profile().unwrap();
        assert!(matches!(
            profile.kind,
            ProfileKind::Local { program: None, args } if args.is_empty()
        ));
    }

    #[test]
    fn update_ssh_profile_password_field() {
        let config = crate::config::AppConfig {
            profiles: vec![SshProfile {
                name: "srv".into(),
                host: "h".into(),
                port: 22,
                user: "u".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: None,
                proxy_command: None,
            }]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        };
        let mut draft = SettingsDraft::from_config(&config);
        assert_eq!(draft.profiles[0].password, "");

        draft.update_profile(0, ProfileField::Password, "newpass".into());
        assert_eq!(draft.profiles[0].password, "newpass");
    }

    #[test]
    fn profile_modal_create_appends_ssh_profile() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_profile_modal();
        draft.set_profile_modal_kind(ProfileDraftKind::Ssh);
        draft.update_profile_modal(ProfileField::Name, "prod".into());
        draft.update_profile_modal(ProfileField::Host, "10.0.0.1".into());
        draft.update_profile_modal(ProfileField::User, "deploy".into());
        draft.save_profile_modal().unwrap();

        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].name, "prod");
        assert_eq!(draft.profiles[0].host, "10.0.0.1");
        assert!(draft.profile_modal_mode.is_none());
    }

    fn ssh_config_host() -> SshProfile {
        SshProfile {
            name: "kube-1".into(),
            host: "192.168.0.230".into(),
            port: 2222,
            user: "root".into(),
            auth_method: SshAuthMethod::KeyFile,
            identity_file: Some("~/.ssh/id_ed25519".into()),
            password: None,
            proxy_command: None,
        }
    }

    #[test]
    fn base_seeds_modal_from_ssh_config_host() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());
        let base = ssh_config_host();

        draft.open_create_profile_modal();
        draft.apply_profile_modal_base(Some(0), Some(&base));

        let modal = &draft.profile_modal_draft;
        assert!(matches!(modal.kind, ProfileDraftKind::Ssh));
        assert_eq!(modal.host, "192.168.0.230");
        assert_eq!(modal.port, "2222");
        assert_eq!(modal.user, "root");
        assert_eq!(modal.identity_file, "~/.ssh/id_ed25519");
        assert_eq!(draft.profile_modal_base, Some(0));
    }

    #[test]
    fn saved_base_profile_is_independent_of_ssh_config() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());
        let base = ssh_config_host();

        draft.open_create_profile_modal();
        draft.apply_profile_modal_base(Some(0), Some(&base));
        draft.update_profile_modal(ProfileField::Host, "10.0.0.9".into());
        draft.save_profile_modal().unwrap();

        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].host, "10.0.0.9");
        assert_eq!(base.host, "192.168.0.230");
    }

    #[test]
    fn clearing_base_resets_connection_fields() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());
        let base = ssh_config_host();

        draft.open_create_profile_modal();
        draft.apply_profile_modal_base(Some(0), Some(&base));
        draft.apply_profile_modal_base(None, None);

        let modal = &draft.profile_modal_draft;
        assert!(matches!(modal.kind, ProfileDraftKind::Ssh));
        assert!(modal.host.is_empty());
        assert!(modal.user.is_empty());
        assert_eq!(draft.profile_modal_base, None);
    }

    #[test]
    fn switching_profile_type_returns_to_first_tab() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_profile_modal();
        draft.set_profile_modal_tab(ProfileModalTab::Advanced);
        draft.set_profile_modal_kind(ProfileDraftKind::Local);

        assert_eq!(draft.profile_modal_tab, ProfileModalTab::Connection);
    }

    #[test]
    fn profile_modal_create_appends_local_profile() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_profile_modal();
        draft.update_profile_modal(ProfileField::Name, "scratch".into());
        draft.update_profile_modal(ProfileField::Program, "/bin/bash".into());
        draft.save_profile_modal().unwrap();

        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].kind, ProfileDraftKind::Local);
        assert_eq!(draft.profiles[0].program, "/bin/bash");
    }

    #[test]
    fn profile_modal_edit_replaces_selected_profile() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            profiles: vec![SshProfile {
                name: "old".into(),
                host: "old.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::KeyFile,
                identity_file: Some("~/.ssh/id_ed25519".into()),
                password: None,
                proxy_command: None,
            }]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        });

        draft.open_edit_profile_modal(0);
        draft.update_profile_modal(ProfileField::Name, "new".into());
        draft.update_profile_modal(ProfileField::Host, "new.example.com".into());
        draft.save_profile_modal().unwrap();

        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].name, "new");
        assert_eq!(draft.profiles[0].host, "new.example.com");
        assert_eq!(draft.profiles[0].identity_file, "~/.ssh/id_ed25519");
    }

    #[test]
    fn profile_modal_cancel_leaves_profiles_unchanged() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            profiles: vec![SshProfile {
                name: "prod".into(),
                host: "prod.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret".into()),
                proxy_command: None,
            }]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        });

        draft.open_edit_profile_modal(0);
        draft.update_profile_modal(ProfileField::Host, "changed.example.com".into());
        draft.close_profile_modal();

        assert_eq!(draft.profiles[0].host, "prod.example.com");
        assert!(draft.profile_modal_mode.is_none());
    }

    #[test]
    fn profile_modal_save_requires_host_for_ssh() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_profile_modal();
        draft.set_profile_modal_kind(ProfileDraftKind::Ssh);
        draft.update_profile_modal(ProfileField::Name, "missing-host".into());
        let err = draft.save_profile_modal().unwrap_err();

        assert_eq!(err, crate::t!("settings.ssh.status.host_required_save"));
        assert!(draft.profiles.is_empty());
        assert!(draft.profile_modal_mode.is_some());
    }

    #[test]
    fn profile_delete_requires_confirmation() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            profiles: vec![
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
            ]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        });

        draft.request_delete_profile(0);

        assert_eq!(draft.profile_delete_pending, Some(0));
        assert_eq!(draft.profiles.len(), 2);

        let removed = draft.confirm_delete_profile().unwrap();

        assert_eq!(removed.host, "prod.example.com");
        assert_eq!(removed.user, "deploy");
        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].host, "stage.example.com");
        assert!(draft.profile_delete_pending.is_none());
    }

    #[test]
    fn profile_delete_cancel_leaves_profile_unchanged() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            profiles: vec![SshProfile {
                name: "prod".into(),
                host: "prod.example.com".into(),
                port: 22,
                user: "deploy".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret".into()),
                proxy_command: None,
            }]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        });

        draft.request_delete_profile(0);
        draft.cancel_delete_profile();

        assert_eq!(draft.profiles.len(), 1);
        assert_eq!(draft.profiles[0].host, "prod.example.com");
        assert!(draft.profile_delete_pending.is_none());
    }

    #[test]
    fn ssh_connection_test_requires_host() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig::default());

        draft.open_create_profile_modal();
        draft.set_profile_modal_kind(ProfileDraftKind::Ssh);
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

        draft.open_create_profile_modal();
        draft.set_profile_modal_kind(ProfileDraftKind::Ssh);
        draft.update_profile_modal(ProfileField::Host, "example.com".into());
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
    fn collect_profiles_skips_blank_draft_and_keeps_valid_ones() {
        let mut draft = SettingsDraft::from_config(&crate::config::AppConfig {
            profiles: vec![SshProfile {
                name: "existing".into(),
                host: "existing.host".into(),
                port: 22,
                user: "u".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: None,
                proxy_command: None,
            }]
            .into_iter()
            .map(crate::gui::tab::Profile::ssh)
            .collect(),
            ..Default::default()
        });
        draft.add_profile();

        let profiles = draft.collect_profiles();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].ssh_profile().unwrap().host, "existing.host");
    }
}
