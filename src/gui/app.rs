use crate::gui::components::{button_primary, button_secondary, panel, text_input_default};
use crate::gui::render::RenderConfig;
use crate::gui::tab::{TerminalSession, TerminalTab};
use iced::widget::text::LineHeight;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Length, Subscription, Task, time};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(usize),
    InputChanged(String),
    SubmitInput,
    Tick,
}

pub struct App {
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    render: RenderConfig,
}

impl App {
    pub fn new() -> Self {
        let render = RenderConfig::default();
        let tabs = vec![
            TerminalTab::zsh(),
            TerminalTab::cmd(),
            TerminalTab::powershell(),
        ];
        Self {
            tabs,
            active_tab: 0,
            render,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(index) if index < self.tabs.len() => {
                self.active_tab = index;
            }
            Message::InputChanged(input) => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.input = input;
                }
            }
            Message::SubmitInput => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab)
                    && let TerminalSession::Active(session) = &tab.session
                    && !tab.input.is_empty()
                {
                    if let Err(err) = session.send_line(&tab.input) {
                        tab.session = TerminalSession::Failed(err.to_string());
                    }
                    tab.input.clear();
                }
            }
            Message::Tick => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.pull_output();
                }
            }
            _ => {}
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let tab_buttons: Vec<Element<Message>> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let label = tab.title.as_str();
                if index == self.active_tab {
                    button_primary(label)
                        .on_press(Message::TabSelected(index))
                        .into()
                } else {
                    button_secondary(label)
                        .on_press(Message::TabSelected(index))
                        .into()
                }
            })
            .collect();

        let tab_row = row(tab_buttons).spacing(8).padding(8);

        let active_tab = &self.tabs[self.active_tab];
        let status_text = active_tab.status_text();
        let rendered = active_tab.rendered_text();
        let dims = active_tab.size();

        let scroll = scrollable(
            text(rendered)
                .size(15)
                .line_height(LineHeight::Relative(1.2))
                .font(iced::font::Font::MONOSPACE), // Use monospace font
        )
        .height(Length::Fill)
        .width(Length::Fill);

        let input_bar = text_input_default("type and hit enter", &active_tab.input)
            .on_input(Message::InputChanged)
            .on_submit(Message::SubmitInput);

        let send_button = button_primary("Send").on_press(Message::SubmitInput);

        let input_row = row(vec![input_bar.into(), send_button.into()]).spacing(8);

        let content = column(vec![
            text(format!("Shell: {}", active_tab.shell)).into(),
            text(format!("Renderer backend: {:?}", self.render.backend)).into(),
            text(format!(
                "Grid: {} cols x {} lines",
                dims.columns, dims.lines
            ))
            .into(),
            text(status_text).size(13).into(),
            scroll.into(),
            input_row.into(),
        ])
        .spacing(8)
        .padding(12);

        panel(column(vec![tab_row.into(), content.into()]).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        time::every(Duration::from_millis(30)).map(|_| Message::Tick)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
