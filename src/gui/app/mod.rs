use crate::config::AppConfig;
use crate::gui::settings::{
    ProfileField, ProfileModalTab, SettingsCategory, SettingsDraft, SettingsField,
    TerminalFontOption,
};
use crate::gui::tab::{Profile, TerminalTab, discover_available_shells};
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
    CreateTab(Profile),
    Settings(SettingsMessage),
    LaunchFromHistory(usize),
    DuplicateTab,
    Sftp(SftpMessage),
    SshPasswordPromptChanged(String),
    SshPasswordPromptToggleSave(bool),
    SshPasswordPromptSubmit,
    SshPasswordPromptCancel,
    ShowTabContextMenu(usize),
    CloseTabContextMenu,
    TerminalRightClick(u64),
    CloseTerminalContextMenu,
    TerminalContextPaste,
    TerminalContextCopy,
    OpenUrl(String),
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
    SelectionChanged {
        pane: u64,
        selection: Option<crate::terminal::Selection>,
    },
    TerminalMousePress {
        pane: u64,
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
    PaneScrollTo {
        pane: u64,
        rel: f32,
    },
    TerminalWheelScroll(f32),

    WindowResized(Size),
    TerminalAreaResized(Size),
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
    BoldIsBrightToggled(bool),
    BellModeSelected(crate::config::BellMode),
    RightClickActionSelected(crate::config::RightClickAction),
    FontSelected(TerminalFontOption),
    ToggleShowAllFonts(bool),

    AddProfile,
    EditProfile(usize),
    LaunchProfile(usize),
    RequestRemoveProfile(usize),
    CancelRemoveProfile,
    ConfirmRemoveProfile,
    ProfileTemplateSelected(usize),
    ProfileModalFieldChanged(ProfileField, String),
    ProfileModalTabSelected(ProfileModalTab),
    TestSshConnection,
    SshConnectionTestFinished(Result<(), String>),
    CloseProfileModal,
    SaveProfileModal,

    #[cfg(target_os = "macos")]
    ConfirmRestartForBlur,
    #[cfg(target_os = "macos")]
    CancelRestartForBlur,
}

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
    pub(super) terminal_area: Size,
    pub(super) settings_open: bool,
    pub(super) settings_category: SettingsCategory,
    pub(super) settings_draft: SettingsDraft,
    pub(super) font_combo_state: combo_box::State<TerminalFontOption>,
    pub(super) show_all_fonts: bool,
    pub(super) all_font_options: Vec<TerminalFontOption>,
    pub(super) available_shells: Vec<Profile>,
    pub(super) config: AppConfig,
    pub(super) pty_sender: Option<mpsc::UnboundedSender<OutputEvent>>,
    pub(super) initial_shell_opened: bool,
    pub(super) next_tab_id: u64,
    pub(super) tab_bar_scroll_x: f32,
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
    pub(super) modal_anim: Animation<bool>,
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
            terminal_area: Size::new(0.0, 0.0),
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
            modal_anim: Animation::new(false)
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

    pub(super) fn grid_for_rect(&self, rect: iced::Rectangle) -> (usize, usize) {
        let pad_x = self.config.terminal.padding_x * 2.0;
        let pad_y = self.config.terminal.padding_y * 2.0;
        let cell_width = self.config.terminal.cell_width.max(1.0);
        let cell_height = self.config.terminal.cell_height.max(1.0);
        let cols = ((rect.width - pad_x).max(1.0) / cell_width) as usize;
        let rows = ((rect.height - pad_y).max(1.0) / cell_height) as usize;
        (cols.max(10), rows.max(5))
    }

    pub(super) fn terminal_area_rect(&self) -> iced::Rectangle {
        let (width, height) = if self.terminal_area.width > 1.0 {
            (self.terminal_area.width, self.terminal_area.height)
        } else {
            (
                (self.window_size.width - 20.0).max(100.0),
                (self.window_size.height - 80.0).max(100.0),
            )
        };
        iced::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    pub(super) fn grid_for_size(&self, size: Size) -> (usize, usize) {
        self.grid_for_rect(iced::Rectangle {
            x: 0.0,
            y: 0.0,
            width: (size.width - 20.0).max(100.0),
            height: (size.height - 80.0).max(100.0),
        })
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

    #[test]
    fn dismissing_the_picker_rewinds_its_animation() {
        let mut app = App::new(AppConfig::default());
        let now = iced::time::Instant::now();

        app.show_shell_picker = true;
        app.modal_anim.go_mut(true, now);
        assert!(app.modal_anim.value());

        app.dismiss_shell_picker();

        assert!(!app.show_shell_picker);
        assert!(!app.modal_anim.value());
    }

    #[test]
    fn profile_templates_start_with_blank_ssh_and_local() {
        let app = App::new(AppConfig::default());
        let templates = app.profile_templates();

        assert!(templates.len() >= 2);
        assert!(matches!(
            templates[0].draft.kind,
            crate::gui::settings::ProfileDraftKind::Ssh
        ));
        assert!(templates[0].draft.host.is_empty());
        assert!(matches!(
            templates[1].draft.kind,
            crate::gui::settings::ProfileDraftKind::Local
        ));
    }

    #[test]
    fn ssh_config_hosts_become_templates() {
        let mut app = App::new(AppConfig::default());
        app.ssh_config_profiles = vec![crate::config::SshProfile {
            name: "kube-1".into(),
            host: "192.168.0.230".into(),
            port: 2222,
            user: "root".into(),
            ..Default::default()
        }];

        let seeded = app
            .profile_templates()
            .into_iter()
            .find(|t| t.draft.name == "kube-1")
            .expect("ssh config host should be offered as a template");

        assert_eq!(seeded.group, crate::gui::settings::TemplateGroup::SshConfig);
        assert_eq!(seeded.draft.host, "192.168.0.230");
        assert_eq!(seeded.draft.port, "2222");
    }

    fn ssh(name: &str) -> crate::config::SshProfile {
        crate::config::SshProfile {
            name: name.into(),
            host: format!("{name}.example.com"),
            port: 22,
            user: "root".into(),
            ..Default::default()
        }
    }

    #[test]
    fn ssh_config_hosts_sit_below_user_profiles() {
        use crate::gui::app::update::tab::PickerSection;

        let mut app = App::new(AppConfig {
            profiles: vec![Profile::ssh(ssh("mine"))],
            ..Default::default()
        });
        app.ssh_config_profiles = vec![ssh("from-config")];

        let sections: Vec<_> = app
            .shell_picker_entries()
            .into_iter()
            .map(|e| (e.section, e.label))
            .collect();

        let ssh_section: Vec<_> = sections
            .iter()
            .filter(|(s, _)| *s == PickerSection::Ssh)
            .map(|(_, l)| l.as_str())
            .collect();
        let config_section: Vec<_> = sections
            .iter()
            .filter(|(s, _)| *s == PickerSection::SshConfig)
            .map(|(_, l)| l.as_str())
            .collect();

        assert_eq!(ssh_section, vec!["mine"]);
        assert_eq!(config_section, vec!["from-config"]);
    }

    #[test]
    fn a_user_profile_shadows_the_ssh_config_host_it_shares_a_name_with() {
        let mut app = App::new(AppConfig {
            profiles: vec![Profile::ssh(ssh("kube-1"))],
            ..Default::default()
        });
        app.ssh_config_profiles = vec![ssh("kube-1")];

        let named: Vec<_> = app
            .shell_picker_entries()
            .into_iter()
            .filter(|e| e.label == "kube-1")
            .collect();

        assert_eq!(named.len(), 1);
    }

    #[test]
    fn picker_selection_maps_to_the_entry_at_that_position() {
        let mut app = App::new(AppConfig {
            profiles: vec![Profile::ssh(ssh("first"))],
            ..Default::default()
        });
        app.ssh_config_profiles = vec![ssh("second")];

        let entries = app.shell_picker_entries();
        assert_eq!(app.shell_picker_option_count(), entries.len());

        app.shell_picker_selected = 1;
        let expected = entries[1].label.clone();
        assert_eq!(
            app.shell_picker_entries()[app.shell_picker_selected].label,
            expected
        );
    }

    fn app_with_pty() -> App {
        let mut app = App::new(AppConfig::default());
        let (tx, _rx) = mpsc::unbounded();
        app.pty_sender = Some(tx);
        app
    }

    #[test]
    fn a_split_shortcut_actually_creates_a_pane() {
        let mut app = app_with_pty();
        let _ = app.update(Message::CreateTab(Profile::default_shell()));
        assert_eq!(app.tabs.len(), 1, "tab was not created");
        assert_eq!(app.tabs[0].panes.len(), 1);

        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::SHIFT
        };
        let _ = app.update(Message::KeyPressed {
            key: Key::Character("E".into()),
            modifiers,
            text: None,
        });

        assert_eq!(
            app.tabs[0].panes.len(),
            2,
            "split shortcut did not add a pane"
        );
        assert_eq!(app.tabs[0].layout.leaves().len(), 2);
    }

    #[test]
    fn clicking_another_pane_moves_focus() {
        let mut app = app_with_pty();
        let _ = app.update(Message::CreateTab(Profile::default_shell()));
        let _ = app.split_focused(crate::gui::pane::Axis::Vertical);

        let ids = app.tabs[0].layout.leaves();
        assert_eq!(ids.len(), 2, "split did not happen");
        let (first, second) = (ids[0], ids[1]);
        assert_eq!(app.tabs[0].focused, second);

        let _ = app.update(Message::SelectionChanged {
            pane: first,
            selection: None,
        });
        assert_eq!(app.tabs[0].focused, first, "click did not move focus");

        let _ = app.update(Message::TerminalRightClick(second));
        assert_eq!(
            app.tabs[0].focused, second,
            "right click did not move focus"
        );
    }

    #[test]
    fn focus_shortcut_moves_between_panes() {
        let mut app = app_with_pty();
        let _ = app.update(Message::CreateTab(Profile::default_shell()));
        app.terminal_area = Size::new(1000.0, 600.0);
        let _ = app.split_focused(crate::gui::pane::Axis::Vertical);

        let ids = app.tabs[0].layout.leaves();
        let (left, right) = (ids[0], ids[1]);
        assert_eq!(app.tabs[0].focused, right);

        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::ALT
        } else {
            Modifiers::CTRL | Modifiers::ALT
        };
        let _ = app.update(Message::KeyPressed {
            key: Key::Named(iced::keyboard::key::Named::ArrowLeft),
            modifiers,
            text: None,
        });
        assert_eq!(
            app.tabs[0].focused, left,
            "focus shortcut did not move left"
        );
    }

    #[test]
    fn repeated_auto_splits_keep_panes_usable() {
        let mut app = app_with_pty();
        let _ = app.update(Message::CreateTab(Profile::default_shell()));
        app.terminal_area = Size::new(1200.0, 700.0);

        let modifiers = if cfg!(target_os = "macos") {
            Modifiers::LOGO | Modifiers::SHIFT
        } else {
            Modifiers::CTRL | Modifiers::SHIFT
        };
        for _ in 0..3 {
            let _ = app.update(Message::KeyPressed {
                key: Key::Character("E".into()),
                modifiers,
                text: None,
            });
        }

        let regions = app.tabs[0].layout.regions(app.terminal_area_rect());
        assert_eq!(regions.len(), 4);
        for (_, rect) in &regions {
            assert!(
                rect.width > 250.0 && rect.height > 150.0,
                "{rect:?} collapsed; splits are not alternating"
            );
        }
    }

    #[test]
    fn each_pane_scrolls_on_its_own() {
        let mut app = app_with_pty();
        let _ = app.update(Message::CreateTab(Profile::default_shell()));
        app.terminal_area = Size::new(1000.0, 600.0);
        let _ = app.split_focused(crate::gui::pane::Axis::Vertical);

        let ids = app.tabs[0].layout.leaves();
        let (left, right) = (ids[0], ids[1]);

        for _ in 0..200 {
            app.tabs[0].pane_mut(left).unwrap().feed_bytes(b"line\r\n");
        }
        assert!(app.tabs[0].pane_mut(left).unwrap().scroll_position().1 > 0);

        let _ = app.update(Message::PaneScrollTo {
            pane: left,
            rel: 0.0,
        });

        assert!(
            app.tabs[0].pane_mut(left).unwrap().scroll_position().0 > 0,
            "scrollbar did not scroll its own pane"
        );
        assert_eq!(
            app.tabs[0].pane_mut(right).unwrap().scroll_position().0,
            0,
            "scrolling one pane moved another"
        );
    }
}
