use crate::gui::components::{button_primary, button_secondary, panel, tab_bar};
use crate::gui::tab::{ShellKind, TerminalTab};
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::text::LineHeight;
use iced::widget::{center, column, container, mouse_area, scrollable, stack, text};
use iced::{Element, Event, Length, Size, Subscription, Task, event, time, window};
use std::time::Duration;

// 셀당 픽셀 크기 (모노스페이스 폰트 기준 대략적인 값)
const CELL_WIDTH: f32 = 9.0;
const CELL_HEIGHT: f32 = 18.0;

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
    Exit,
}

pub struct App {
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    show_shell_picker: bool,
    window_size: Size,
}

impl App {
    pub fn new() -> Self {
        let tabs = vec![];
        Self {
            tabs,
            active_tab: 0,
            show_shell_picker: false,
            window_size: Size::new(800.0, 600.0),
        }
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
                let terminal_height = (self.window_size.height - 80.0).max(100.0);
                let terminal_width = (self.window_size.width - 20.0).max(100.0);
                let cols = (terminal_width / CELL_WIDTH) as usize;
                let rows = (terminal_height / CELL_HEIGHT) as usize;

                let new_tab = TerminalTab::from_shell(shell, cols.max(10), rows.max(5));
                self.tabs.push(new_tab);
                self.active_tab = self.tabs.len() - 1;
                self.show_shell_picker = false;
            }
            Message::Tick => {
                // 현재 탭의 출력 가져오기
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.pull_output();
                }

                // 죽은 탭들 제거
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
                return window::get_latest().and_then(window::close);
            }
            Message::WindowResized(size) => {
                self.window_size = size;
                // 터미널 영역 계산 (탭바, 상태바, 패딩 등 제외)
                let terminal_height = (size.height - 80.0).max(100.0);
                let terminal_width = (size.width - 20.0).max(100.0);

                let cols = (terminal_width / CELL_WIDTH) as usize;
                let rows = (terminal_height / CELL_HEIGHT) as usize;

                // 모든 탭 리사이즈
                for tab in &mut self.tabs {
                    tab.resize(cols.max(10), rows.max(5));
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
                let status_text = active_tab.status_text();
                let rendered = active_tab.rendered_text();
                let dims = active_tab.size();

                let scroll = scrollable(
                    text(rendered)
                        .size(15)
                        .line_height(LineHeight::Relative(1.2))
                        .font(iced::font::Font::MONOSPACE),
                )
                .height(Length::Fill)
                .width(Length::Fill);

                column(vec![
                    text(format!(
                        "{}  |  {}x{}  |  {}",
                        active_tab.shell, dims.columns, dims.lines, status_text
                    ))
                    .size(12)
                    .into(),
                    scroll.into(),
                ])
                .spacing(4)
                .padding(8)
                .into()
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
        let base_layout = panel(column(vec![tab_row, main_content]).height(Length::Fill))
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
                    button_secondary("cmd")
                        .on_press(Message::CreateTab(ShellKind::Cmd))
                        .width(Length::Fill)
                        .into(),
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
        Self::new()
    }
}
