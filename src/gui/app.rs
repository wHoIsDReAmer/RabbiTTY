use crate::config::AppConfig;
use crate::gui::settings::{
    SettingsCategory, SettingsDraft, SettingsField, SshProfileField, TerminalFontOption,
};
use crate::gui::tab::{ShellKind, TerminalTab};
use crate::session::OutputEvent;
use crate::terminal_font::discover_system_terminal_fonts;
use iced::Size;
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers};

#[path = "app/shortcuts.rs"]
mod shortcuts;
#[path = "app/update.rs"]
pub(crate) mod update;
#[path = "app/view.rs"]
mod view;

pub(super) const SETTINGS_TAB_INDEX: usize = usize::MAX;

#[derive(Clone)]
pub enum Message {
    TabSelected(usize),
    CloseTab(usize),
    OpenShellPicker,
    CloseShellPicker,
    CreateTab(ShellKind),
    OpenSettingsTab,
    SelectSettingsCategory(SettingsCategory),
    SettingsInputChanged(SettingsField, String),
    SettingsBlurToggled(bool),
    ApplySettings,
    SaveSettings,
    AddSshProfile,
    RemoveSshProfile(usize),
    SshProfileFieldChanged(usize, SshProfileField, String),
    SaveSshProfiles,
    CreateSshTab(usize),
    #[cfg(target_os = "macos")]
    ConfirmRestartForBlur,
    #[cfg(target_os = "macos")]
    CancelRestartForBlur,
    PtySenderReady(mpsc::Sender<OutputEvent>),
    PtyOutput(OutputEvent),
    PtyOutputBatch(Vec<OutputEvent>),
    KeyPressed {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    },
    TabBarScroll(f32),
    WindowResized(Size),
    ApplyWindowStyle,
    #[cfg(target_os = "windows")]
    WindowMinimize,
    #[cfg(target_os = "windows")]
    WindowMaximize,
    #[cfg(target_os = "windows")]
    WindowDrag,
    Exit,
}

pub struct App {
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    show_shell_picker: bool,
    shell_picker_selected: usize,
    window_size: Size,
    settings_open: bool,
    settings_category: SettingsCategory,
    settings_draft: SettingsDraft,
    terminal_font_options: Vec<TerminalFontOption>,
    config: AppConfig,
    pty_sender: Option<mpsc::Sender<OutputEvent>>,
    next_tab_id: u64,
    tab_bar_scroll_offset: f32,
    window_style_applied: bool,
    #[cfg(target_os = "macos")]
    show_restart_confirm: bool,
    #[cfg(target_os = "macos")]
    pending_settings_updates: Option<crate::config::AppConfigUpdates>,
    #[cfg(target_os = "macos")]
    pending_save_on_restart: bool,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            tabs: vec![],
            active_tab: 0,
            show_shell_picker: false,
            shell_picker_selected: 0,
            window_size: Size::new(config.ui.window_width, config.ui.window_height),
            settings_open: false,
            settings_category: SettingsCategory::Ui,
            settings_draft: SettingsDraft::from_config(&config),
            terminal_font_options: build_terminal_font_options(
                config.terminal.font_selection.as_deref(),
            ),
            config,
            pty_sender: None,
            next_tab_id: 1,
            tab_bar_scroll_offset: 0.0,
            window_style_applied: false,
            #[cfg(target_os = "macos")]
            show_restart_confirm: false,
            #[cfg(target_os = "macos")]
            pending_settings_updates: None,
            #[cfg(target_os = "macos")]
            pending_save_on_restart: false,
        }
    }

    fn grid_for_size(&self, size: Size) -> (usize, usize) {
        let pad_x = self.config.terminal.padding_x * 2.0;
        let pad_y = self.config.terminal.padding_y * 2.0;
        let terminal_height = (size.height - 80.0 - pad_y).max(100.0);
        let terminal_width = (size.width - 20.0 - pad_x).max(100.0);
        let cell_width = self.config.terminal.cell_width.max(1.0);
        let cell_height = self.config.terminal.cell_height.max(1.0);
        let cols = (terminal_width / cell_width) as usize;
        let rows = (terminal_height / cell_height) as usize;
        (cols.max(10), rows.max(5))
    }

    pub fn window_style(&self) -> iced::theme::Style {
        iced::theme::Style {
            background_color: self.theme_background_color(),
            text_color: self.theme_text_color(),
        }
    }

    fn theme_background_color(&self) -> iced::Color {
        theme_color(
            self.config.theme.background,
            self.config.theme.background_opacity,
        )
    }

    fn theme_text_color(&self) -> iced::Color {
        theme_color(self.config.theme.foreground, 1.0)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new(AppConfig::default())
    }
}

pub(super) fn theme_color(rgb: [u8; 3], alpha: f32) -> iced::Color {
    iced::Color::from_linear_rgba(
        srgb_u8_to_linear(rgb[0]),
        srgb_u8_to_linear(rgb[1]),
        srgb_u8_to_linear(rgb[2]),
        alpha,
    )
}

pub(super) fn srgb_u8_to_linear(value: u8) -> f32 {
    let v = f32::from(value) / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn build_terminal_font_options(selected: Option<&str>) -> Vec<TerminalFontOption> {
    let mut options = Vec::new();
    options.push(TerminalFontOption {
        label: "DejaVu Sans Mono (Bundled)".to_string(),
        value: String::new(),
    });

    options.extend(
        discover_system_terminal_fonts()
            .into_iter()
            .map(|family| TerminalFontOption {
                label: family.clone(),
                value: family,
            }),
    );

    if let Some(value) = selected.map(str::trim).filter(|value| !value.is_empty())
        && !options.iter().any(|option| option.value == value)
    {
        options.push(TerminalFontOption {
            label: format!("{value} (Legacy)"),
            value: value.to_string(),
        });
    }

    options
}
