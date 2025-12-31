use crate::config::AppConfig;
#[cfg(target_family = "unix")]
use crate::gui::components::{button_primary, button_secondary, panel, tab_bar};
use crate::gui::render::TerminalProgram;
use crate::gui::tab::{ShellKind, TerminalTab};
use crate::terminal::TerminalTheme;
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::{center, column, container, mouse_area, stack, text};
use iced::{Element, Event, Length, Size, Subscription, Task, event, time, window};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(usize),
    CloseTab(usize),
    OpenShellPicker,
    CloseShellPicker,
    CreateTab(ShellKind),
    Tick,
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
    config: AppConfig,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        let tabs = vec![];
        Self {
            tabs,
            active_tab: 0,
            show_shell_picker: false,
            window_size: Size::new(config.ui.window_width, config.ui.window_height),
            config,
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
        theme_color(self.config.theme.background, 1.0)
    }

    fn theme_text_color(&self) -> iced::Color {
        theme_color(self.config.theme.foreground, 1.0)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) if index < self.tabs.len() => {
                self.active_tab = index;
            }
            Message::CloseTab(index) => {
                if index < self.tabs.len() {
                    self.tabs.remove(index);

                    if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                        self.active_tab = self.tabs.len() - 1;
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
                let (cols, rows) = self.grid_for_size(self.window_size);
                let theme = TerminalTheme::from_config(&self.config);
                let new_tab = TerminalTab::from_shell(shell, cols, rows, theme);
                self.tabs.push(new_tab);
                self.active_tab = self.tabs.len() - 1;
                self.show_shell_picker = false;
            }
            Message::Tick => {
                // Get current tab outputs
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.pull_output();
                }

                // Remove died tabs
                let mut i = 0;
                while i < self.tabs.len() {
                    if !self.tabs[i].is_alive() {
                        self.tabs.remove(i);
                        if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
                            self.active_tab = self.tabs.len() - 1;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            Message::KeyPressed {
                key,
                modifiers,
                text,
            } => {
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
                return window::latest().and_then(window::close);
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
            _ => {}
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tabs_iter = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| (tab.title.as_str(), i, i == self.active_tab));
        let tab_row = tab_bar(tabs_iter, Message::OpenShellPicker);

        // Main contents
        let main_content: Element<Message> =
            if let Some(active_tab) = self.tabs.get(self.active_tab) {
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

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Ticking
            time::every(Duration::from_millis(30)).map(|_| Message::Tick),
            // Iced events (maybe will be added?)
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
