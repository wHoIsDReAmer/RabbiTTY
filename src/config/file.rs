use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use super::AppConfig;
use super::defaults::*;
use super::types::{BellMode, CursorShape, RightClickAction, TabBarPosition};
use crate::gui::tab::Profile;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct FileConfig {
    pub(super) ui: Option<UiFileConfig>,
    pub(super) terminal: Option<TerminalFileConfig>,
    pub(super) theme: Option<ThemeFileConfig>,
    pub(super) shortcuts: Option<ShortcutsFileConfig>,
    #[serde(default)]
    pub(super) profiles: Option<Vec<Profile>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct UiFileConfig {
    pub(super) window_width: Option<f32>,
    pub(super) window_height: Option<f32>,
    pub(super) language: Option<String>,
    pub(super) animations_enabled: Option<bool>,
    pub(super) tab_bar_position: Option<TabBarPosition>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct TerminalFileConfig {
    pub(super) cell_width: Option<f32>,
    pub(super) cell_height: Option<f32>,
    pub(super) font_selection: Option<String>,
    #[serde(alias = "font_path")]
    pub(super) legacy_font_path: Option<String>,
    pub(super) font_size: Option<f32>,
    pub(super) padding_x: Option<f32>,
    pub(super) padding_y: Option<f32>,
    pub(super) scrollback_lines: Option<usize>,
    pub(super) bracketed_paste: Option<bool>,
    pub(super) multiline_paste_confirm: Option<bool>,
    pub(super) scroll_multiplier: Option<f32>,
    pub(super) cursor_shape: Option<CursorShape>,
    pub(super) cursor_blink: Option<bool>,
    pub(super) bold_is_bright: Option<bool>,
    pub(super) bell_mode: Option<BellMode>,
    pub(super) right_click_action: Option<RightClickAction>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ThemeFileConfig {
    pub(super) color_scheme: Option<String>,
    pub(super) foreground: Option<String>,
    pub(super) background: Option<String>,
    pub(super) cursor: Option<String>,
    pub(super) ansi_colors: Option<Vec<String>>,
    pub(super) background_opacity: Option<f32>,
    pub(super) blur_enabled: Option<bool>,
    pub(super) macos_blur_radius: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ShortcutsFileConfig {
    pub(super) new_tab: Option<String>,
    pub(super) close_tab: Option<String>,
    pub(super) open_settings: Option<String>,
    pub(super) next_tab: Option<String>,
    pub(super) prev_tab: Option<String>,
    pub(super) quit: Option<String>,
    pub(super) font_size_increase: Option<String>,
    pub(super) font_size_decrease: Option<String>,
    pub(super) font_size_reset: Option<String>,
    pub(super) duplicate_tab: Option<String>,
}

impl From<&AppConfig> for FileConfig {
    fn from(config: &AppConfig) -> Self {
        Self {
            ui: Some(UiFileConfig {
                window_width: Some(config.ui.window_width),
                window_height: Some(config.ui.window_height),
                language: config.ui.language.clone(),
                animations_enabled: Some(config.ui.animations_enabled),
                tab_bar_position: Some(config.ui.tab_bar_position),
            }),
            terminal: Some(TerminalFileConfig {
                cell_width: None,
                cell_height: None,
                font_selection: config.terminal.font_selection.clone(),
                legacy_font_path: None,
                font_size: Some(config.terminal.font_size),
                padding_x: Some(config.terminal.padding_x),
                padding_y: Some(config.terminal.padding_y),
                scrollback_lines: Some(config.terminal.scrollback_lines),
                bracketed_paste: Some(config.terminal.bracketed_paste),
                multiline_paste_confirm: Some(config.terminal.multiline_paste_confirm),
                scroll_multiplier: Some(config.terminal.scroll_multiplier),
                cursor_shape: Some(config.terminal.cursor_shape),
                cursor_blink: Some(config.terminal.cursor_blink),
                bold_is_bright: Some(config.terminal.bold_is_bright),
                bell_mode: Some(config.terminal.bell_mode),
                right_click_action: Some(config.terminal.right_click_action),
            }),
            theme: Some(ThemeFileConfig {
                color_scheme: if config.theme.color_scheme.is_empty() {
                    None
                } else {
                    Some(config.theme.color_scheme.clone())
                },
                foreground: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.foreground[0],
                    config.theme.foreground[1],
                    config.theme.foreground[2]
                )),
                background: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.background[0],
                    config.theme.background[1],
                    config.theme.background[2]
                )),
                cursor: Some(format!(
                    "#{:02x}{:02x}{:02x}",
                    config.theme.cursor[0], config.theme.cursor[1], config.theme.cursor[2]
                )),
                ansi_colors: config.theme.ansi_colors.map(|ansi| {
                    ansi.iter()
                        .map(|c| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]))
                        .collect()
                }),
                background_opacity: Some(config.theme.background_opacity),
                blur_enabled: Some(config.theme.blur_enabled),
                macos_blur_radius: Some(config.theme.macos_blur_radius),
            }),
            shortcuts: Some(ShortcutsFileConfig {
                new_tab: Some(config.shortcuts.new_tab.clone()),
                close_tab: Some(config.shortcuts.close_tab.clone()),
                open_settings: Some(config.shortcuts.open_settings.clone()),
                next_tab: Some(config.shortcuts.next_tab.clone()),
                prev_tab: Some(config.shortcuts.prev_tab.clone()),
                quit: Some(config.shortcuts.quit.clone()),
                font_size_increase: Some(config.shortcuts.font_size_increase.clone()),
                font_size_decrease: Some(config.shortcuts.font_size_decrease.clone()),
                font_size_reset: Some(config.shortcuts.font_size_reset.clone()),
                duplicate_tab: Some(config.shortcuts.duplicate_tab.clone()),
            }),
            profiles: if config.profiles.is_empty() {
                None
            } else {
                Some(
                    config
                        .profiles
                        .iter()
                        .filter(|p| p.ssh_profile().is_none_or(|s| !s.host.trim().is_empty()))
                        .cloned()
                        .collect(),
                )
            },
        }
    }
}

pub(super) fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("rabbitty").join("config.toml"))
}

pub(super) fn ensure_config_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, default_config_toml().as_bytes())?;
    Ok(())
}

pub(super) fn default_config_toml() -> String {
    format!(
        "[ui]\nwindow_width = {width}\nwindow_height = {height}\n\n[terminal]\nfont_selection = \"\"\nfont_size = {font_size:.1}\npadding_x = {padding_x:.1}\npadding_y = {padding_y:.1}\nbell_mode = \"sound\"\n\n[theme]\ncolor_scheme = \"Catppuccin Mocha\"\nforeground = \"#{fg:02x}{fg_g:02x}{fg_b:02x}\"\nbackground = \"#{bg:02x}{bg_g:02x}{bg_b:02x}\"\ncursor = \"#{cur:02x}{cur_g:02x}{cur_b:02x}\"\nbackground_opacity = {opacity:.2}\nblur_enabled = {blur_enabled}\nmacos_blur_radius = {macos_blur_radius}\n\n[shortcuts]\nnew_tab = \"{shortcut_new_tab}\"\nclose_tab = \"{shortcut_close_tab}\"\nopen_settings = \"{shortcut_open_settings}\"\nnext_tab = \"{shortcut_next_tab}\"\nprev_tab = \"{shortcut_prev_tab}\"\nquit = \"{shortcut_quit}\"\nfont_size_increase = \"{shortcut_font_size_increase}\"\nfont_size_decrease = \"{shortcut_font_size_decrease}\"\nfont_size_reset = \"{shortcut_font_size_reset}\"\nduplicate_tab = \"{shortcut_duplicate_tab}\"\n",
        width = DEFAULT_WINDOW_WIDTH as u32,
        height = DEFAULT_WINDOW_HEIGHT as u32,
        font_size = DEFAULT_TERMINAL_FONT_SIZE,
        padding_x = DEFAULT_TERMINAL_PADDING_X,
        padding_y = DEFAULT_TERMINAL_PADDING_Y,
        fg = DEFAULT_THEME_FOREGROUND[0],
        fg_g = DEFAULT_THEME_FOREGROUND[1],
        fg_b = DEFAULT_THEME_FOREGROUND[2],
        bg = DEFAULT_THEME_BACKGROUND[0],
        bg_g = DEFAULT_THEME_BACKGROUND[1],
        bg_b = DEFAULT_THEME_BACKGROUND[2],
        cur = DEFAULT_THEME_CURSOR[0],
        cur_g = DEFAULT_THEME_CURSOR[1],
        cur_b = DEFAULT_THEME_CURSOR[2],
        opacity = DEFAULT_THEME_BG_OPACITY,
        blur_enabled = DEFAULT_BLUR_ENABLED,
        macos_blur_radius = DEFAULT_MACOS_BLUR_RADIUS,
        shortcut_new_tab = DEFAULT_SHORTCUT_NEW_TAB,
        shortcut_close_tab = DEFAULT_SHORTCUT_CLOSE_TAB,
        shortcut_open_settings = DEFAULT_SHORTCUT_OPEN_SETTINGS,
        shortcut_next_tab = DEFAULT_SHORTCUT_NEXT_TAB,
        shortcut_prev_tab = DEFAULT_SHORTCUT_PREV_TAB,
        shortcut_quit = DEFAULT_SHORTCUT_QUIT,
        shortcut_font_size_increase = DEFAULT_SHORTCUT_FONT_SIZE_INCREASE,
        shortcut_font_size_decrease = DEFAULT_SHORTCUT_FONT_SIZE_DECREASE,
        shortcut_font_size_reset = DEFAULT_SHORTCUT_FONT_SIZE_RESET,
        shortcut_duplicate_tab = DEFAULT_SHORTCUT_DUPLICATE_TAB
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{SshAuthMethod, SshProfile};

    #[test]
    fn generated_default_config_exposes_sound_bell_mode() {
        let toml_str = default_config_toml();
        let file = toml::from_str::<FileConfig>(&toml_str).expect("default config should parse");

        assert!(toml_str.contains("bell_mode = \"sound\""));
        assert_eq!(
            file.terminal.and_then(|terminal| terminal.bell_mode),
            Some(BellMode::Sound)
        );
    }

    #[test]
    fn profiles_parse_ssh_and_skip_empty_host() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [[profiles]]
            name = "My Server"
            [profiles.kind]
            type = "ssh"
            host = "example.com"
            port = 2222
            user = "admin"
            auth_method = "key_file"
            identity_file = "~/.ssh/id_ed25519"

            [[profiles]]
            name = "No Host"
            [profiles.kind]
            type = "ssh"
            host = "  "
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        let sshs = config.ssh_profiles();
        assert_eq!(sshs.len(), 1); // empty-host profile dropped
        assert_eq!(sshs[0].host, "example.com");
        assert_eq!(sshs[0].port, 2222);
        assert_eq!(sshs[0].auth_method, crate::config::SshAuthMethod::KeyFile);
        assert!(sshs[0].password.is_none());
    }

    #[test]
    fn ssh_profile_serialization_excludes_password() {
        let config = AppConfig {
            profiles: vec![crate::gui::tab::Profile::ssh(SshProfile {
                name: "test".into(),
                host: "host.com".into(),
                port: 22,
                user: "user".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: Some("secret123".into()),
                proxy_command: None,
            })],
            ..Default::default()
        };

        let file = FileConfig::from(&config);
        let toml_str = toml::to_string_pretty(&file).unwrap();
        assert!(!toml_str.contains("secret123"));
        assert!(!toml_str.contains("password ="));
    }

    #[test]
    fn ssh_profile_serialization_skips_empty_host_profiles() {
        let config = AppConfig {
            profiles: vec![crate::gui::tab::Profile::ssh(SshProfile {
                name: "".into(),
                host: "".into(),
                port: 22,
                user: "".into(),
                auth_method: SshAuthMethod::Password,
                identity_file: None,
                password: None,
                proxy_command: None,
            })],
            ..Default::default()
        };

        let file = FileConfig::from(&config);
        let toml_str = toml::to_string_pretty(&file).unwrap();

        assert!(!toml_str.contains("[[profiles]]"));
    }

    #[test]
    fn file_config_terminal_font_selection_supports_legacy_alias() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [terminal]
            font_path = "  Legacy Font  "
            font_size = 15.0
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert_eq!(
            config.terminal.font_selection,
            Some("Legacy Font".to_string())
        );
        assert_eq!(config.terminal.font_size, 15.0);
    }

    #[test]
    fn file_config_terminal_font_selection_prefers_new_key() {
        let mut config = AppConfig::default();
        let file = toml::from_str::<FileConfig>(
            r#"
            [terminal]
            font_selection = " JetBrains Mono "
            font_path = " Legacy Font "
            "#,
        )
        .expect("file config should parse");

        config.apply_file(file);
        assert_eq!(
            config.terminal.font_selection,
            Some("JetBrains Mono".to_string())
        );
    }
}
