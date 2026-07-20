use crate::config::AppConfig;
use crate::gui::settings::{
    SettingsCategory, SettingsDraft, SettingsField, SshProfileField, TerminalFontOption,
};
use crate::gui::tab::{ShellKind, TerminalTab, discover_available_shells};
use crate::session::OutputEvent;
use crate::session::history::SessionHistory;
use crate::terminal::font::discover_system_terminal_fonts;
use iced::Animation;
use iced::Size;
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers};
use iced::widget::combo_box;
use std::sync::mpsc as std_mpsc;

mod shortcuts;
mod subscription;
pub(crate) mod update;
mod view;

pub(super) const SETTINGS_TAB_INDEX: usize = usize::MAX;

#[derive(Clone)]
pub enum Message {
    Noop,
    TabSelected(usize),
    TabDragHover(usize),
    TabDragRelease,
    CloseTab(usize),
    OpenShellPicker,
    CloseShellPicker,
    CreateTab(ShellKind),
    Settings(SettingsMessage),
    CreateSshTab(usize),
    LaunchFromHistory(usize),
    DuplicateTab,
    Sftp(SftpMessage),
    SshPasswordPromptChanged(String),
    SshPasswordPromptToggleSave(bool),
    SshPasswordPromptSubmit,
    SshPasswordPromptCancel,
    CreateSshTabFromConfig(usize),
    ShowTabContextMenu(usize),
    CloseTabContextMenu,
    TerminalRightClick,
    CloseTerminalContextMenu,
    TerminalContextPaste,
    TerminalContextCopy,
    CursorMoved(iced::Point),
    PtySenderReady(mpsc::UnboundedSender<OutputEvent>),
    PtyOutput(OutputEvent),
    PtyOutputBatch(Vec<OutputEvent>),
    KeyPressed {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    },

    TabBarScroll(f32),
    TabBarScrolled(f32),
    SelectionChanged(Option<crate::terminal::Selection>),
    TerminalMousePress {
        col: usize,
        row: usize,
    },
    TerminalMouseRelease {
        col: usize,
        row: usize,
    },
    TerminalMouseDrag {
        col: usize,
        row: usize,
    },
    TerminalSelectionAutoscroll {
        up: bool,
        col: usize,
    },
    TerminalSelectionAutoscrollStop,
    SelectionAutoscrollTick,
    PasteClipboard(String),
    ConfirmMultilinePaste,
    CancelMultilinePaste,
    ImeStateChanged(bool),
    ImeCommit(String),
    ImePreedit(String, Option<std::ops::Range<usize>>),
    TerminalScroll(f32),
    TerminalWheelScroll(f32),

    WindowResized(Size),
    ResizeDebounce,
    AnimationTick,
    CursorBlink,
    ApplyWindowStyle,

    #[cfg(target_os = "windows")]
    WindowMinimize,
    #[cfg(target_os = "windows")]
    WindowMaximize,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    WindowDrag,
    Exit,
}

/// Messages driving the settings screen and the SSH profile modal.
/// Handled by [`App::update_settings_message`].
#[derive(Clone)]
pub enum SettingsMessage {
    OpenTab,
    SelectCategory(SettingsCategory),
    InputChanged(SettingsField, String),
    InputCommitted(SettingsField, String),
    CommitDebounce,
    BlurToggled(bool),
    AnimationsToggled(bool),
    TabBarPositionSelected(crate::config::TabBarPosition),
    BracketedPasteToggled(bool),
    MultilinePasteConfirmToggled(bool),
    CursorShapeSelected(crate::config::CursorShape),
    CursorBlinkToggled(bool),
    BellModeSelected(crate::config::BellMode),
    RightClickActionSelected(crate::config::RightClickAction),
    FontSelected(TerminalFontOption),
    ToggleShowAllFonts(bool),

    AddSshProfile,
    EditSshProfile(usize),
    RequestRemoveSshProfile(usize),
    CancelRemoveSshProfile,
    ConfirmRemoveSshProfile,
    SshProfileModalFieldChanged(SshProfileField, String),
    TestSshConnection,
    SshConnectionTestFinished(Result<(), String>),
    CloseSshProfileModal,
    SaveSshProfileModal,

    #[cfg(target_os = "macos")]
    ConfirmRestartForBlur,
    #[cfg(target_os = "macos")]
    CancelRestartForBlur,
}

/// Messages driving the SFTP drawer. Handled by [`App::update_sftp`].
#[derive(Clone)]
pub enum SftpMessage {
    ToggleDrawer,
    OpenSucceeded {
        tab_id: u64,
        command_tx: iced::futures::channel::mpsc::UnboundedSender<crate::ssh::sftp::Command>,
    },
    OpenFailed {
        tab_id: u64,
        error: String,
    },
    Event {
        tab_id: u64,
        event: crate::ssh::sftp::Event,
    },
    Navigate {
        tab_id: u64,
        path: String,
    },
    Refresh,
    RequestUpload,
    UploadPicked {
        tab_id: u64,
        files: Vec<std::path::PathBuf>,
    },
    RequestDownload {
        tab_id: u64,
        remote: String,
        suggested_name: String,
    },
    DownloadPicked {
        tab_id: u64,
        remote: String,
        local: std::path::PathBuf,
    },
    CancelTransfer,
    DismissTransfer {
        tab_id: u64,
        path: String,
    },
}

pub struct App {
    pub(super) tabs: Vec<TerminalTab>,
    pub(super) active_tab: usize,
    pub(super) show_shell_picker: bool,
    pub(super) shell_picker_selected: usize,
    pub(super) window_size: Size,
    pub(super) settings_open: bool,
    pub(super) settings_category: SettingsCategory,
    pub(super) settings_draft: SettingsDraft,
    pub(super) font_combo_state: combo_box::State<TerminalFontOption>,
    pub(super) show_all_fonts: bool,
    pub(super) all_font_options: Vec<TerminalFontOption>,
    pub(super) available_shells: Vec<ShellKind>,
    pub(super) config: AppConfig,
    pub(super) pty_sender: Option<mpsc::UnboundedSender<OutputEvent>>,
    pub(super) initial_shell_opened: bool,
    pub(super) next_tab_id: u64,
    pub(super) tab_bar_scroll_x: f32,
    pub(super) ignore_scrollable_sync: u8,
    pub(super) scroll_follow_bottom: bool,
    pub(super) dragging_tab: Option<usize>,
    pub(super) drag_target: Option<usize>,
    pub(super) scroll_accumulator: f32,
    // Some(true) = autoscroll up, Some(false) = down, None = off.
    pub(super) selection_autoscroll: Option<bool>,
    pub(super) selection_autoscroll_col: usize,
    pub(super) resize_debounce_pending: bool,
    pub(super) resize_debounce_seq: u64,
    pub(super) resize_debounce_spawned_seq: u64,
    pub(super) settings_debounce_pending: bool,
    pub(super) settings_debounce_seq: u64,
    pub(super) settings_debounce_spawned_seq: u64,
    pub(super) shell_picker_anim: Animation<bool>,
    pub(super) settings_category_transition: crate::gui::components::CategoryTransition,
    pub(super) palette: crate::gui::theme::Palette,
    pub(super) ime_active: bool,
    pub(super) ime_preedit: Option<(String, Option<std::ops::Range<usize>>)>,
    pub(super) session_history: SessionHistory,
    pub(super) window_style_applied: bool,
    pub(super) tab_context_menu: Option<usize>,
    /// Whether the terminal right-click context menu is currently shown.
    pub(super) terminal_context_menu: bool,
    pub(super) cursor_position: iced::Point,
    #[cfg(target_os = "macos")]
    pub(super) show_restart_confirm: bool,
    #[cfg(target_os = "macos")]
    pub(super) pending_settings_updates: Option<crate::config::AppConfigUpdates>,
    #[cfg(target_os = "macos")]
    pub(super) pending_save_on_restart: bool,
    pub(super) config_save_tx: std_mpsc::Sender<AppConfig>,
    /// Profiles parsed from `~/.ssh/config`, merged into shell/SSH lists at
    /// runtime so users do not have to re-enter them in Settings.
    pub(super) ssh_config_profiles: Vec<crate::config::SshProfile>,
    /// In-flight password prompt deferred from an SSH tab creation.
    pub(super) password_prompt: Option<PasswordPromptState>,
    /// Text waiting for multiline-paste confirmation.
    pub(super) pending_paste: Option<String>,
    /// Current on/off phase of the blinking cursor.
    pub(super) cursor_blink_on: bool,
    /// Start time of an active visual bell flash, if any.
    pub(super) bell_flash_start: Option<std::time::Instant>,
}

/// Duration of the visual bell flash overlay.
pub(super) const BELL_FLASH_DURATION: std::time::Duration = std::time::Duration::from_millis(150);

#[derive(Debug, Clone)]
pub struct PasswordPromptState {
    pub profile: crate::config::SshProfile,
    pub draft: String,
    pub save_to_keychain: bool,
    pub error: Option<String>,
}

fn spawn_config_save_worker() -> std_mpsc::Sender<AppConfig> {
    let (tx, rx) = std_mpsc::channel::<AppConfig>();
    std::thread::spawn(move || {
        while let Ok(mut latest) = rx.recv() {
            while let Ok(newer) = rx.try_recv() {
                latest = newer;
            }
            if let Err(err) = latest.save() {
                eprintln!("Failed to save config: {err}");
            }
        }
    });
    tx
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let palette = crate::gui::theme::Palette::from_theme(&config.theme);
        let all_font_options = build_all_font_options(config.terminal.font_selection.as_deref());
        let show_all_fonts = false;
        let font_combo_state = build_font_combo_state(
            &all_font_options,
            show_all_fonts,
            config.terminal.font_selection.as_deref(),
        );
        Self {
            tabs: vec![],
            active_tab: 0,
            show_shell_picker: false,
            shell_picker_selected: 0,
            window_size: Size::new(config.ui.window_width, config.ui.window_height),
            settings_open: false,
            settings_category: SettingsCategory::Appearance,
            settings_draft: SettingsDraft::from_config(&config),
            font_combo_state,
            show_all_fonts,
            all_font_options,
            available_shells: discover_available_shells(),
            config,
            pty_sender: None,
            initial_shell_opened: false,
            next_tab_id: 1,
            tab_bar_scroll_x: 0.0,
            ignore_scrollable_sync: 0,
            scroll_follow_bottom: true,
            dragging_tab: None,
            drag_target: None,
            scroll_accumulator: 0.0,
            selection_autoscroll: None,
            selection_autoscroll_col: 0,
            resize_debounce_pending: false,
            resize_debounce_seq: 0,
            resize_debounce_spawned_seq: 0,
            settings_debounce_pending: false,
            settings_debounce_seq: 0,
            settings_debounce_spawned_seq: 0,
            session_history: SessionHistory::load(),
            palette,
            tab_context_menu: None,
            terminal_context_menu: false,
            cursor_position: iced::Point::ORIGIN,
            ime_active: false,
            ime_preedit: None,
            shell_picker_anim: Animation::new(false)
                .duration(std::time::Duration::from_millis(250))
                .easing(iced::animation::Easing::EaseOutQuint),
            settings_category_transition: crate::gui::components::CategoryTransition::new(),
            window_style_applied: false,
            #[cfg(target_os = "macos")]
            show_restart_confirm: false,
            #[cfg(target_os = "macos")]
            pending_settings_updates: None,
            #[cfg(target_os = "macos")]
            pending_save_on_restart: false,
            config_save_tx: spawn_config_save_worker(),
            ssh_config_profiles: crate::ssh::user_config::load(),
            password_prompt: None,
            pending_paste: None,
            cursor_blink_on: true,
            bell_flash_start: None,
        }
    }

    pub(super) fn grid_for_size(&self, size: Size) -> (usize, usize) {
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

    pub(super) fn theme_background_color(&self) -> iced::Color {
        theme_color(
            self.config.theme.background,
            self.config.theme.background_opacity,
        )
    }

    pub(super) fn theme_text_color(&self) -> iced::Color {
        theme_color(self.config.theme.foreground, 1.0)
    }

    pub(in crate::gui) fn take_initial_shell_request(&mut self) -> bool {
        if self.initial_shell_opened {
            return false;
        }

        self.initial_shell_opened = true;
        true
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

/// Build the full list of font options (monospaced + proportional).
fn build_all_font_options(selected: Option<&str>) -> Vec<TerminalFontOption> {
    let mut options = Vec::new();
    options.push(TerminalFontOption {
        label: "DejaVu Sans Mono (Bundled)".to_string(),
        value: String::new(),
        monospaced: true,
    });

    options.extend(discover_system_terminal_fonts().into_iter().map(|sf| {
        let label = if sf.monospaced {
            sf.family.clone()
        } else {
            format!("{} (Proportional)", sf.family)
        };
        TerminalFontOption {
            label,
            value: sf.family,
            monospaced: sf.monospaced,
        }
    }));

    if let Some(value) = selected.map(str::trim).filter(|value| !value.is_empty())
        && !options.iter().any(|option| option.value == value)
    {
        options.push(TerminalFontOption {
            label: format!("{value} (Legacy)"),
            value: value.to_string(),
            monospaced: true,
        });
    }

    options
}

/// Build combo_box state from the font options, filtered by show_all flag.
fn build_font_combo_state(
    all: &[TerminalFontOption],
    show_all: bool,
    selected_value: Option<&str>,
) -> combo_box::State<TerminalFontOption> {
    let filtered: Vec<TerminalFontOption> = if show_all {
        all.to_vec()
    } else {
        all.iter().filter(|o| o.monospaced).cloned().collect()
    };
    let selection_idx = selected_value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .and_then(|v| filtered.iter().position(|o| o.value == v));
    let selection = selection_idx.map(|i| &filtered[i]);
    combo_box::State::with_selection(filtered.clone(), selection)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_shell_request_is_consumed_once() {
        let mut app = App::new(AppConfig::default());

        assert!(app.take_initial_shell_request());
        assert!(!app.take_initial_shell_request());
    }
}
