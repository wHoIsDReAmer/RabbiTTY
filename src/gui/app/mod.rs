use crate::config::AppConfig;
use crate::gui::settings::{
    SettingsCategory, SettingsDraft, SettingsField, SshProfileField, TerminalFontOption,
};
use crate::gui::tab::{ShellKind, TerminalTab, discover_available_shells};
use crate::session::OutputEvent;
use crate::session_history::SessionHistory;
use crate::terminal::font::discover_system_terminal_fonts;
use iced::Animation;
use iced::Size;
use iced::futures::channel::mpsc;
use iced::keyboard::{Key, Modifiers};
use iced::widget::combo_box;

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
    OpenSettingsTab,
    SelectSettingsCategory(SettingsCategory),
    SettingsInputChanged(SettingsField, String),
    SettingsBlurToggled(bool),
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
    CreateSshTab(usize),
    LaunchFromHistory(usize),
    DuplicateTab,
    ShowTabContextMenu(usize),
    CloseTabContextMenu,
    CursorMoved(iced::Point),
    #[cfg(target_os = "macos")]
    ConfirmRestartForBlur,
    #[cfg(target_os = "macos")]
    CancelRestartForBlur,
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
    PasteClipboard(String),
    ImeStateChanged(bool),
    ImeCommit(String),
    ImePreedit(String, Option<std::ops::Range<usize>>),
    TerminalScroll(f32),
    TerminalWheelScroll(f32),

    WindowResized(Size),
    ResizeDebounce,
    AnimationTick,
    ApplyWindowStyle,

    FontSelected(TerminalFontOption),
    ToggleShowAllFonts(bool),

    #[cfg(target_os = "windows")]
    WindowMinimize,
    #[cfg(target_os = "windows")]
    WindowMaximize,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    WindowDrag,
    Exit,
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
    pub(super) resize_debounce_pending: bool,
    pub(super) resize_debounce_seq: u64,
    pub(super) resize_debounce_spawned_seq: u64,
    pub(super) shell_picker_anim: Animation<bool>,
    pub(super) palette: crate::gui::theme::Palette,
    pub(super) ime_active: bool,
    pub(super) ime_preedit: Option<(String, Option<std::ops::Range<usize>>)>,
    pub(super) session_history: SessionHistory,
    pub(super) window_style_applied: bool,
    pub(super) tab_context_menu: Option<usize>,
    pub(super) cursor_position: iced::Point,
    #[cfg(target_os = "macos")]
    pub(super) show_restart_confirm: bool,
    #[cfg(target_os = "macos")]
    pub(super) pending_settings_updates: Option<crate::config::AppConfigUpdates>,
    #[cfg(target_os = "macos")]
    pub(super) pending_save_on_restart: bool,
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
            settings_category: SettingsCategory::Ui,
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
            resize_debounce_pending: false,
            resize_debounce_seq: 0,
            resize_debounce_spawned_seq: 0,
            session_history: SessionHistory::load(),
            palette,
            tab_context_menu: None,
            cursor_position: iced::Point::ORIGIN,
            ime_active: false,
            ime_preedit: None,
            shell_picker_anim: Animation::new(false)
                .duration(std::time::Duration::from_millis(250))
                .easing(iced::animation::Easing::EaseOutQuint),
            window_style_applied: false,
            #[cfg(target_os = "macos")]
            show_restart_confirm: false,
            #[cfg(target_os = "macos")]
            pending_settings_updates: None,
            #[cfg(target_os = "macos")]
            pending_save_on_restart: false,
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
