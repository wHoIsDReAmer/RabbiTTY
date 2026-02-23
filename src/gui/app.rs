use crate::config::{AppConfig, AppConfigUpdates, ShortcutsConfig};
use crate::gui::components::{button_primary, button_secondary, panel, tab_bar};
use crate::gui::render::TerminalProgram;
use crate::gui::settings::{self, SettingsCategory, SettingsDraft, SettingsField};
use crate::gui::tab::{ShellKind, TerminalTab};
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL};
use crate::session::OutputEvent;
use crate::terminal::TerminalTheme;
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::sink::SinkExt;
use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::stream;
use iced::widget::{button, center, column, container, mouse_area, row, scrollable, stack, text};
use iced::{
    Background, Border, Color, Element, Event, Length, Size, Subscription, Task, event, window,
};

const SETTINGS_TAB_INDEX: usize = usize::MAX;

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
    #[cfg(target_os = "macos")]
    ConfirmRestartForBlur,
    #[cfg(target_os = "macos")]
    CancelRestartForBlur,
    PtySenderReady(mpsc::Sender<OutputEvent>),
    PtyOutput(OutputEvent),
    KeyPressed {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    },
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
    config: AppConfig,
    pty_sender: Option<mpsc::Sender<OutputEvent>>,
    next_tab_id: u64,
    window_style_applied: bool,
    #[cfg(target_os = "macos")]
    show_restart_confirm: bool,
    #[cfg(target_os = "macos")]
    pending_settings_updates: Option<AppConfigUpdates>,
    #[cfg(target_os = "macos")]
    pending_save_on_restart: bool,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let tabs = vec![];
        Self {
            tabs,
            active_tab: 0,
            show_shell_picker: false,
            shell_picker_selected: 0,
            window_size: Size::new(config.ui.window_width, config.ui.window_height),
            settings_open: false,
            settings_category: SettingsCategory::Ui,
            settings_draft: SettingsDraft::from_config(&config),
            config,
            pty_sender: None,
            next_tab_id: 1,
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
        let terminal_height = (size.height - 80.0).max(100.0);
        let terminal_width = (size.width - 20.0).max(100.0);
        let cell_width = self.config.terminal.cell_width.max(1.0);
        let cell_height = self.config.terminal.cell_height.max(1.0);
        let cols = (terminal_width / cell_width) as usize;
        let rows = (terminal_height / cell_height) as usize;
        (cols.max(10), rows.max(5))
    }

    pub fn window_style(&self) -> iced::theme::Style {
        let background_color = self.theme_background_color();

        iced::theme::Style {
            background_color,
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

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) => {
                if index == SETTINGS_TAB_INDEX && self.settings_open {
                    self.active_tab = SETTINGS_TAB_INDEX;
                } else if index < self.tabs.len() {
                    self.active_tab = index;
                }
            }
            Message::CloseTab(index) => {
                if index == SETTINGS_TAB_INDEX {
                    self.settings_open = false;
                    if self.active_tab == SETTINGS_TAB_INDEX {
                        self.active_tab = self.tabs.len().saturating_sub(1);
                    }
                } else if index < self.tabs.len() {
                    self.tabs.remove(index);

                    if self.active_tab != SETTINGS_TAB_INDEX {
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                        if self.tabs.is_empty() {
                            self.active_tab = 0;
                        }
                    }
                }
            }
            Message::OpenShellPicker => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
            }
            Message::CloseShellPicker => {
                self.show_shell_picker = false;
                self.shell_picker_selected = 0;
            }
            Message::CreateTab(shell) => {
                return self.create_tab(shell);
            }
            Message::OpenSettingsTab => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
            }
            Message::SelectSettingsCategory(category) => {
                self.settings_category = category;
                if !self.settings_open {
                    self.settings_open = true;
                    self.active_tab = SETTINGS_TAB_INDEX;
                    self.settings_draft = SettingsDraft::from_config(&self.config);
                }
            }
            Message::SettingsInputChanged(field, value) => {
                self.settings_draft.update(field, value);
            }
            Message::SettingsBlurToggled(enabled) => {
                self.settings_draft.blur_enabled = enabled;
            }
            Message::ApplySettings => {
                return self.apply_settings(false);
            }
            Message::SaveSettings => {
                return self.apply_settings(true);
            }
            #[cfg(target_os = "macos")]
            Message::ConfirmRestartForBlur => {
                if let Some(updates) = self.pending_settings_updates.take() {
                    let _ = self.apply_updates_to_runtime(updates);
                    if self.pending_save_on_restart
                        && let Err(err) = self.config.save()
                    {
                        eprintln!("Failed to save config: {err}");
                    }
                }

                let restart_spawned = match std::env::current_exe() {
                    Ok(current_exe) => {
                        let args: Vec<_> = std::env::args_os().skip(1).collect();
                        match std::process::Command::new(current_exe).args(args).spawn() {
                            Ok(_) => true,
                            Err(err) => {
                                eprintln!("Failed to relaunch app: {err}");
                                false
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("Failed to locate executable for restart: {err}");
                        false
                    }
                };

                self.show_restart_confirm = false;
                self.pending_save_on_restart = false;

                if restart_spawned {
                    return iced::exit();
                }

                return Task::none();
            }
            #[cfg(target_os = "macos")]
            Message::CancelRestartForBlur => {
                self.show_restart_confirm = false;
                self.pending_settings_updates = None;
                self.pending_save_on_restart = false;
                return Task::none();
            }
            Message::PtySenderReady(sender) => {
                self.pty_sender = Some(sender);
            }
            Message::PtyOutput(event) => match event {
                OutputEvent::Data { tab_id, bytes } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        tab.feed_bytes(&bytes);
                    }
                }
                OutputEvent::Closed { tab_id } => {
                    if let Some(index) = self.tabs.iter().position(|t| t.id == tab_id) {
                        self.tabs.remove(index);
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                    }
                }
            },
            Message::KeyPressed {
                key,
                modifiers,
                text,
            } => {
                if self.show_shell_picker {
                    match key {
                        Key::Named(Named::Escape) => {
                            self.show_shell_picker = false;
                            self.shell_picker_selected = 0;
                        }
                        Key::Named(Named::ArrowUp) => {
                            self.shift_shell_picker_selection(-1);
                        }
                        Key::Named(Named::ArrowDown) => {
                            self.shift_shell_picker_selection(1);
                        }
                        Key::Named(Named::Enter) => {
                            return self.confirm_shell_picker_selection();
                        }
                        _ => {}
                    }
                    return Task::none();
                }

                if let Some(task) = self.handle_app_shortcut(&key, modifiers) {
                    return task;
                }

                if self.active_tab == SETTINGS_TAB_INDEX {
                    return Task::none();
                }
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.handle_key(&key, modifiers, text.as_deref());
                }
            }
            Message::Exit => {
                return iced::exit();
            }
            Message::ApplyWindowStyle => {
                if self.window_style_applied {
                    return Task::none();
                }
                self.window_style_applied = true;

                #[cfg(any(target_os = "windows", target_os = "macos"))]
                {
                    let theme = self.config.theme.clone();
                    return window::latest()
                        .and_then(move |id| {
                            let theme = theme.clone();
                            window::run(id, move |window| {
                                if let Ok(handle) = window.window_handle() {
                                    crate::platform::apply_style(handle, &theme);
                                }
                            })
                        })
                        .discard();
                }

                #[cfg(not(any(target_os = "windows", target_os = "macos")))]
                {
                    return Task::none();
                }
            }
            #[cfg(target_os = "windows")]
            Message::WindowMinimize => {
                return window::latest().and_then(|id| window::minimize(id, true));
            }
            #[cfg(target_os = "windows")]
            Message::WindowMaximize => {
                return window::latest().and_then(window::toggle_maximize);
            }
            #[cfg(target_os = "windows")]
            Message::WindowDrag => {
                return window::latest().and_then(window::drag);
            }

            Message::WindowResized(size) => {
                self.window_size = size;
                let (cols, rows) = self.grid_for_size(size);

                for tab in &mut self.tabs {
                    tab.resize(cols, rows);
                }
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        self.view_main()
    }

    fn view_main(&self) -> Element<'_, Message> {
        let tabs_iter = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.title.as_str(), i, i == self.active_tab));
        let settings_iter = self
            .settings_open
            .then_some((
                "Settings",
                SETTINGS_TAB_INDEX,
                self.active_tab == SETTINGS_TAB_INDEX,
            ))
            .into_iter();
        let tabs_iter = tabs_iter.chain(settings_iter);
        let ui_alpha = self.config.theme.background_opacity;
        let bar_alpha = (ui_alpha * 0.9).clamp(0.0, 1.0);
        let tab_alpha = (ui_alpha * 0.6).clamp(0.0, 1.0);
        let tab_row = tab_bar(
            tabs_iter,
            Message::OpenShellPicker,
            Message::OpenSettingsTab,
            bar_alpha,
            tab_alpha,
        );

        // Main contents
        let main_content: Element<Message> = if self.active_tab == SETTINGS_TAB_INDEX {
            self.view_config()
        } else if let Some(active_tab) = self.tabs.get(self.active_tab) {
            let dims = active_tab.size();
            let cells = active_tab.render_cells();
            let grid_size = dims;
            let terminal_stack = TerminalProgram { cells, grid_size }
                .widget()
                .width(Length::Fill)
                .height(Length::Fill);

            terminal_stack.into()
        } else {
            column(vec![
                text("No tabs open").size(20).into(),
                text("Click + to create a new tab").size(14).into(),
            ])
            .spacing(8)
            .padding(20)
            .into()
        };

        // Base layout
        let panel_background = Some(self.theme_background_color());
        let base_layout = panel(
            column(vec![tab_row, main_content]).height(Length::Fill),
            panel_background,
            self.theme_text_color(),
        )
        .width(Length::Fill)
        .height(Length::Fill);

        #[cfg(target_os = "macos")]
        if self.show_restart_confirm {
            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.4,
                        })),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CancelRestartForBlur);

            let popup_card = container(
                column(vec![
                    text("Blur on macOS requires restart.").size(16).into(),
                    text("Save changes and restart now?").size(13).into(),
                    row(vec![
                        button_secondary("Cancel")
                            .on_press(Message::CancelRestartForBlur)
                            .into(),
                        button_primary("Save & Restart")
                            .on_press(Message::ConfirmRestartForBlur)
                            .into(),
                    ])
                    .spacing(SPACING_SMALL)
                    .into(),
                ])
                .spacing(SPACING_NORMAL)
                .padding(20)
                .width(Length::Fixed(300.0)),
            )
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::color!(0x31, 0x32, 0x44))),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: iced::color!(0x45, 0x47, 0x5a),
                },
                ..Default::default()
            });

            let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

            return stack![base_layout, backdrop, centered_popup,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into();
        }

        // Popup
        if self.show_shell_picker {
            // Transparent backdrop (click to close)
            let backdrop = mouse_area(
                container(text(""))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(|_theme: &iced::Theme| container::Style {
                        background: Some(iced::Background::Color(iced::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.4,
                        })),
                        ..Default::default()
                    }),
            )
            .on_press(Message::CloseShellPicker);

            // Popup card
            let popup_card = container(
                column(vec![
                    #[cfg(target_family = "unix")]
                    if self.shell_picker_selected == 0 {
                        button_primary("zsh")
                    } else {
                        button_secondary("zsh")
                    }
                    .on_press(Message::CreateTab(ShellKind::Zsh))
                    .width(Length::Fill)
                    .into(),
                    #[cfg(target_family = "windows")]
                    if self.shell_picker_selected == 0 {
                        button_primary("cmd")
                    } else {
                        button_secondary("cmd")
                    }
                    .on_press(Message::CreateTab(ShellKind::Cmd))
                    .width(Length::Fill)
                    .into(),
                    #[cfg(target_family = "windows")]
                    if self.shell_picker_selected == 1 {
                        button_primary("PowerShell")
                    } else {
                        button_secondary("PowerShell")
                    }
                    .on_press(Message::CreateTab(ShellKind::PowerShell))
                    .width(Length::Fill)
                    .into(),
                    if self.shell_picker_selected == self.shell_picker_option_count() - 1 {
                        button_primary("Cancel")
                    } else {
                        button_secondary("Cancel")
                    }
                    .on_press(Message::CloseShellPicker)
                    .width(Length::Fill)
                    .into(),
                ])
                .spacing(10)
                .padding(20)
                .width(Length::Fixed(220.0)),
            )
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(iced::Background::Color(iced::color!(0x31, 0x32, 0x44))),
                border: iced::Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: iced::color!(0x45, 0x47, 0x5a),
                },
                ..Default::default()
            });

            // Make popup centered
            let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

            stack![base_layout, backdrop, centered_popup,]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            base_layout.into()
        }
    }

    fn view_config(&self) -> Element<'_, Message> {
        let palette = Palette::DARK;
        let mut category_items: Vec<Element<Message>> = Vec::new();

        for category in SettingsCategory::ALL {
            let is_active = category == self.settings_category;
            let label = category.label();
            let button_style = move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                let base_bg = if is_active {
                    Color {
                        a: 0.35,
                        ..palette.background
                    }
                } else {
                    Color::TRANSPARENT
                };
                let hover_bg = if is_active {
                    base_bg
                } else {
                    Color {
                        a: 0.2,
                        ..palette.background
                    }
                };

                let background = match status {
                    iced::widget::button::Status::Hovered => hover_bg,
                    _ => base_bg,
                };

                iced::widget::button::Style {
                    background: Some(Background::Color(background)),
                    text_color: if is_active {
                        palette.text
                    } else {
                        palette.text_secondary
                    },
                    border: Border {
                        radius: RADIUS_NORMAL.into(),
                        width: if is_active { 1.0 } else { 0.0 },
                        color: Color {
                            a: 0.15,
                            ..palette.text
                        },
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            };

            let item = button(text(label).size(13))
                .padding([6, 10])
                .width(Length::Fill)
                .on_press(Message::SelectSettingsCategory(category))
                .style(button_style);
            category_items.push(item.into());
        }

        let sidebar = container(
            column(category_items)
                .spacing(SPACING_SMALL)
                .padding(SPACING_NORMAL)
                .width(Length::Fill),
        )
        .width(Length::Fixed(180.0))
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            ..Default::default()
        });

        let header = row![
            text("Settings").size(18),
            row![
                button_secondary("Apply").on_press(Message::ApplySettings),
                button_primary("Save").on_press(Message::SaveSettings),
            ]
            .spacing(SPACING_SMALL)
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill);

        let body_content = container(settings::view_category(
            self.settings_category,
            &self.config,
            &self.settings_draft,
        ))
        .padding([0, 12])
        .width(Length::Fill);

        let body = scrollable(body_content)
            .height(Length::Fill)
            .width(Length::Fill);

        let content = container(
            column(vec![header.into(), body.into()])
                .spacing(SPACING_NORMAL)
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SPACING_LARGE);

        row![sidebar, content]
            .spacing(SPACING_LARGE)
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    fn apply_settings(&mut self, save: bool) -> Task<Message> {
        let updates = self.settings_draft.to_updates();

        #[cfg(target_os = "macos")]
        if let Some(new_enabled) = updates.blur_enabled
            && new_enabled != self.config.theme.blur_enabled
        {
            self.show_restart_confirm = true;
            self.pending_settings_updates = Some(updates);
            self.pending_save_on_restart = true;
            return Task::none();
        }

        let resize_task = self.apply_updates_to_runtime(updates);

        if save && let Err(err) = self.config.save() {
            eprintln!("Failed to save config: {err}");
        }

        resize_task
    }

    fn apply_updates_to_runtime(&mut self, updates: AppConfigUpdates) -> Task<Message> {
        self.config.apply_updates(updates);
        self.settings_draft = SettingsDraft::from_config(&self.config);

        let new_size = Size::new(self.config.ui.window_width, self.config.ui.window_height);
        let resize_task = if (self.window_size.width - new_size.width).abs() > f32::EPSILON
            || (self.window_size.height - new_size.height).abs() > f32::EPSILON
        {
            self.window_size = new_size;
            window::latest().and_then(move |id| window::resize(id, new_size))
        } else {
            Task::none()
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        for tab in &mut self.tabs {
            tab.resize(cols, rows);
            tab.set_theme(theme.clone());
        }

        resize_task
    }

    fn create_tab(&mut self, shell: ShellKind) -> Task<Message> {
        let Some(sender) = self.pty_sender.clone() else {
            eprintln!("PTY output channel not ready");
            return Task::none();
        };

        let (cols, rows) = self.grid_for_size(self.window_size);
        let theme = TerminalTheme::from_config(&self.config);
        let tab_id = self.next_tab_id;
        self.next_tab_id = self.next_tab_id.wrapping_add(1);
        let new_tab = TerminalTab::from_shell(shell, cols, rows, theme, tab_id, sender);
        self.tabs.push(new_tab);
        self.active_tab = self.tabs.len() - 1;
        self.show_shell_picker = false;
        self.shell_picker_selected = 0;
        Task::none()
    }

    fn shell_picker_option_count(&self) -> usize {
        #[cfg(target_family = "unix")]
        {
            2
        }

        #[cfg(target_family = "windows")]
        {
            3
        }
    }

    fn shift_shell_picker_selection(&mut self, delta: isize) {
        let count = self.shell_picker_option_count() as isize;
        if count <= 0 {
            return;
        }

        let next = (self.shell_picker_selected as isize + delta).rem_euclid(count) as usize;
        self.shell_picker_selected = next;
    }

    fn confirm_shell_picker_selection(&mut self) -> Task<Message> {
        #[cfg(target_family = "unix")]
        {
            match self.shell_picker_selected {
                0 => self.create_tab(ShellKind::Zsh),
                _ => {
                    self.show_shell_picker = false;
                    self.shell_picker_selected = 0;
                    Task::none()
                }
            }
        }

        #[cfg(target_family = "windows")]
        {
            return match self.shell_picker_selected {
                0 => self.create_tab(ShellKind::Cmd),
                1 => self.create_tab(ShellKind::PowerShell),
                _ => {
                    self.show_shell_picker = false;
                    self.shell_picker_selected = 0;
                    Task::none()
                }
            };
        }
    }

    fn handle_app_shortcut(&mut self, key: &Key, modifiers: Modifiers) -> Option<Task<Message>> {
        let action = ShortcutAction::resolve(key, modifiers, &self.config.shortcuts)?;

        match action {
            ShortcutAction::NewTab => {
                self.show_shell_picker = true;
                self.shell_picker_selected = 0;
                Some(Task::none())
            }
            ShortcutAction::CloseTab => {
                self.close_active_target();
                Some(Task::none())
            }
            ShortcutAction::OpenSettings => {
                self.settings_open = true;
                self.active_tab = SETTINGS_TAB_INDEX;
                self.settings_draft = SettingsDraft::from_config(&self.config);
                Some(Task::none())
            }
            ShortcutAction::NextTab => {
                self.select_relative_tab(1);
                Some(Task::none())
            }
            ShortcutAction::PrevTab => {
                self.select_relative_tab(-1);
                Some(Task::none())
            }
            ShortcutAction::Quit => Some(iced::exit()),
        }
    }

    fn close_active_target(&mut self) {
        if self.active_tab == SETTINGS_TAB_INDEX {
            self.settings_open = false;
            self.active_tab = self.tabs.len().saturating_sub(1);
            return;
        }

        if self.tabs.is_empty() {
            return;
        }

        let index = self.active_tab.min(self.tabs.len() - 1);
        self.tabs.remove(index);

        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
            self.active_tab = self.tabs.len() - 1;
        }
        if self.tabs.is_empty() {
            self.active_tab = 0;
        }
    }

    fn select_relative_tab(&mut self, step: isize) {
        let mut visible_tabs: Vec<usize> = (0..self.tabs.len()).collect();
        if self.settings_open {
            visible_tabs.push(SETTINGS_TAB_INDEX);
        }

        if visible_tabs.is_empty() {
            return;
        }

        let current_pos = visible_tabs
            .iter()
            .position(|index| *index == self.active_tab)
            .unwrap_or(0);

        let len = visible_tabs.len() as isize;
        let next_pos = (current_pos as isize + step).rem_euclid(len) as usize;
        self.active_tab = visible_tabs[next_pos];
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            Subscription::run(|| {
                stream::channel(100, async |mut output| {
                    let (sender, mut receiver) = mpsc::channel(100);
                    let _ = output.send(Message::PtySenderReady(sender)).await;

                    while let Some(event) = receiver.next().await {
                        if output.send(Message::PtyOutput(event)).await.is_err() {
                            break;
                        }
                    }
                })
            }),
            event::listen_with(|event, _status, _id| match event {
                Event::Window(window::Event::CloseRequested) => Some(Message::Exit),
                Event::Window(window::Event::Resized(size)) => Some(Message::WindowResized(size)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                }) => Some(Message::KeyPressed {
                    key,
                    modifiers,
                    text: text.map(|s| s.to_string()),
                }),
                _ => None,
            }),
        ])
    }
}

#[derive(Debug, Clone, Copy)]
enum ShortcutAction {
    NewTab,
    CloseTab,
    OpenSettings,
    NextTab,
    PrevTab,
    Quit,
}

impl ShortcutAction {
    fn resolve(key: &Key, modifiers: Modifiers, shortcuts: &ShortcutsConfig) -> Option<Self> {
        if shortcut_matches(&shortcuts.new_tab, key, modifiers) {
            return Some(Self::NewTab);
        }
        if shortcut_matches(&shortcuts.close_tab, key, modifiers) {
            return Some(Self::CloseTab);
        }
        if shortcut_matches(&shortcuts.open_settings, key, modifiers) {
            return Some(Self::OpenSettings);
        }
        if shortcut_matches(&shortcuts.next_tab, key, modifiers) {
            return Some(Self::NextTab);
        }
        if shortcut_matches(&shortcuts.prev_tab, key, modifiers) {
            return Some(Self::PrevTab);
        }
        if shortcut_matches(&shortcuts.quit, key, modifiers) {
            return Some(Self::Quit);
        }
        None
    }
}

#[derive(Debug, Clone)]
struct ParsedShortcut {
    modifiers: Modifiers,
    key: String,
}

fn shortcut_matches(binding: &str, key: &Key, modifiers: Modifiers) -> bool {
    let Some(parsed) = parse_shortcut(binding) else {
        return false;
    };
    let Some(event_key) = event_key_token(key) else {
        return false;
    };

    let tracked = Modifiers::SHIFT | Modifiers::CTRL | Modifiers::ALT | Modifiers::LOGO;
    let pressed = modifiers & tracked;

    parsed.key == event_key && parsed.modifiers == pressed
}

fn parse_shortcut(value: &str) -> Option<ParsedShortcut> {
    let mut modifiers = Modifiers::default();
    let mut key: Option<String> = None;

    for token in value.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }

        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers.insert(Modifiers::CTRL),
            "alt" | "option" => modifiers.insert(Modifiers::ALT),
            "shift" => modifiers.insert(Modifiers::SHIFT),
            "cmd" | "command" | "meta" | "super" => modifiers.insert(Modifiers::COMMAND),
            _ => {
                if key.is_some() {
                    return None;
                }
                key = normalize_shortcut_key_token(token);
                key.as_ref()?;
            }
        }
    }

    Some(ParsedShortcut {
        modifiers,
        key: key?,
    })
}

fn event_key_token(key: &Key) -> Option<String> {
    match key {
        Key::Named(named) => match named {
            Named::Enter => Some("Enter".to_string()),
            Named::Tab => Some("Tab".to_string()),
            Named::Space => Some("Space".to_string()),
            Named::Escape => Some("Escape".to_string()),
            Named::ArrowUp => Some("ArrowUp".to_string()),
            Named::ArrowDown => Some("ArrowDown".to_string()),
            Named::ArrowLeft => Some("ArrowLeft".to_string()),
            Named::ArrowRight => Some("ArrowRight".to_string()),
            Named::Home => Some("Home".to_string()),
            Named::End => Some("End".to_string()),
            Named::Delete => Some("Delete".to_string()),
            Named::Backspace => Some("Backspace".to_string()),
            Named::Insert => Some("Insert".to_string()),
            Named::PageUp => Some("PageUp".to_string()),
            Named::PageDown => Some("PageDown".to_string()),
            Named::F1 => Some("F1".to_string()),
            Named::F2 => Some("F2".to_string()),
            Named::F3 => Some("F3".to_string()),
            Named::F4 => Some("F4".to_string()),
            Named::F5 => Some("F5".to_string()),
            Named::F6 => Some("F6".to_string()),
            Named::F7 => Some("F7".to_string()),
            Named::F8 => Some("F8".to_string()),
            Named::F9 => Some("F9".to_string()),
            Named::F10 => Some("F10".to_string()),
            Named::F11 => Some("F11".to_string()),
            Named::F12 => Some("F12".to_string()),
            _ => None,
        },
        Key::Character(c) => {
            let mut chars = c.chars();
            let ch = chars.next()?;
            if chars.next().is_some() {
                return None;
            }

            if ch.is_ascii_alphabetic() {
                return Some(ch.to_ascii_uppercase().to_string());
            }

            match ch {
                ',' => Some("Comma".to_string()),
                '.' => Some("Period".to_string()),
                _ if ch.is_ascii_digit()
                    || matches!(ch, '[' | ']' | '/' | ';' | '\'' | '-' | '=' | '`') =>
                {
                    Some(ch.to_string())
                }
                _ => None,
            }
        }
        Key::Unidentified => None,
    }
}

fn normalize_shortcut_key_token(value: &str) -> Option<String> {
    let lower = value.trim().to_ascii_lowercase();

    let normalized = match lower.as_str() {
        "esc" | "escape" => "Escape".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "space" | "spacebar" => "Space".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "delete" | "del" => "Delete".to_string(),
        "backspace" => "Backspace".to_string(),
        "insert" | "ins" => "Insert".to_string(),
        "pageup" | "page-up" | "pgup" => "PageUp".to_string(),
        "pagedown" | "page-down" | "pgdown" => "PageDown".to_string(),
        "up" | "arrowup" => "ArrowUp".to_string(),
        "down" | "arrowdown" => "ArrowDown".to_string(),
        "left" | "arrowleft" => "ArrowLeft".to_string(),
        "right" | "arrowright" => "ArrowRight".to_string(),
        "comma" => "Comma".to_string(),
        "period" | "dot" => "Period".to_string(),
        "f1" => "F1".to_string(),
        "f2" => "F2".to_string(),
        "f3" => "F3".to_string(),
        "f4" => "F4".to_string(),
        "f5" => "F5".to_string(),
        "f6" => "F6".to_string(),
        "f7" => "F7".to_string(),
        "f8" => "F8".to_string(),
        "f9" => "F9".to_string(),
        "f10" => "F10".to_string(),
        "f11" => "F11".to_string(),
        "f12" => "F12".to_string(),
        _ => {
            if lower.chars().count() == 1 {
                let ch = lower.chars().next()?;
                if ch.is_ascii_alphanumeric() {
                    ch.to_ascii_uppercase().to_string()
                } else if matches!(
                    ch,
                    ',' | '.' | '[' | ']' | '/' | ';' | '\'' | '-' | '=' | '`'
                ) {
                    ch.to_string()
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };

    Some(normalized)
}

impl Default for App {
    fn default() -> Self {
        Self::new(AppConfig::default())
    }
}

fn theme_color(rgb: [u8; 3], alpha: f32) -> iced::Color {
    iced::Color {
        r: f32::from(rgb[0]) / 255.0,
        g: f32::from(rgb[1]) / 255.0,
        b: f32::from(rgb[2]) / 255.0,
        a: alpha,
    }
}
