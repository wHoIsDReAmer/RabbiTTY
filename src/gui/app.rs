use crate::config::AppConfig;
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
use iced::keyboard::{self, Key, Modifiers};
use iced::stream;
use iced::widget::{button, center, column, container, mouse_area, row, stack, text};
use iced::{Background, Border, Color, Element, Event, Length, Size, Subscription, Task, event, window};

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
    ApplySettings,
    SaveSettings,
    PtySenderReady(mpsc::Sender<OutputEvent>),
    PtyOutput(OutputEvent),
    KeyPressed {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    },
    WindowResized(Size),
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
    window_size: Size,
    settings_open: bool,
    settings_category: SettingsCategory,
    settings_draft: SettingsDraft,
    config: AppConfig,
    pty_sender: Option<mpsc::Sender<OutputEvent>>,
    next_tab_id: u64,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let tabs = vec![];
        Self {
            tabs,
            active_tab: 0,
            show_shell_picker: false,
            window_size: Size::new(config.ui.window_width, config.ui.window_height),
            settings_open: false,
            settings_category: SettingsCategory::Ui,
            settings_draft: SettingsDraft::from_config(&config),
            config,
            pty_sender: None,
            next_tab_id: 1,
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
            }
            Message::CloseShellPicker => {
                self.show_shell_picker = false;
            }
            Message::CreateTab(shell) => {
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
            Message::ApplySettings => {
                return self.apply_settings();
            }
            Message::SaveSettings => {
                let task = self.apply_settings();
                if let Err(err) = self.config.save() {
                    eprintln!("Failed to save config: {err}");
                }
                return task;
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
                if self.active_tab == SETTINGS_TAB_INDEX {
                    return Task::none();
                }
                // If popup is opened
                if self.show_shell_picker {
                    if matches!(key, Key::Named(iced::keyboard::key::Named::Escape)) {
                        self.show_shell_picker = false;
                    }
                    return Task::none();
                }

                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.handle_key(&key, modifiers, text.as_deref());
                }
            }
            Message::Exit => {
                return iced::exit();
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
                    button_primary("zsh")
                        .on_press(Message::CreateTab(ShellKind::Zsh))
                        .width(Length::Fill)
                        .into(),
                    #[cfg(target_family = "windows")]
                    button_secondary("cmd")
                        .on_press(Message::CreateTab(ShellKind::Cmd))
                        .width(Length::Fill)
                        .into(),
                    #[cfg(target_family = "windows")]
                    button_secondary("PowerShell")
                        .on_press(Message::CreateTab(ShellKind::PowerShell))
                        .width(Length::Fill)
                        .into(),
                    button_secondary("Cancel")
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
            border: Border {
                radius: RADIUS_NORMAL.into(),
                width: 1.0,
                color: Color {
                    a: 0.12,
                    ..palette.text
                },
            },
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

        let body = settings::view_category(
            self.settings_category,
            &self.config,
            &self.settings_draft,
        );

        let content = container(
            column(vec![header.into(), body])
                .spacing(SPACING_NORMAL)
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

    fn apply_settings(&mut self) -> Task<Message> {
        let updates = self.settings_draft.to_updates();
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
