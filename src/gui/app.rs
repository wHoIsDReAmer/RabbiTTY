use crate::gui::components::{button_primary, button_secondary, panel};
use crate::gui::render::RenderConfig;
use crate::gui::tab::TerminalTab;
use iced::keyboard::{self, Key, Modifiers};
use iced::widget::text::LineHeight;
use iced::widget::{column, row, scrollable, text};
use iced::{Element, Event, Length, Subscription, Task, event, time};
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(usize),
    Tick,
    KeyPressed {
        key: Key,
        modifiers: Modifiers,
        text: Option<String>,
    },
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
            Message::Tick => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.pull_output();
                }
            }
            Message::KeyPressed {
                key,
                modifiers,
                text,
            } => {
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.handle_key(&key, modifiers, text.as_deref());
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
                .font(iced::font::Font::MONOSPACE),
        )
        .height(Length::Fill)
        .width(Length::Fill);

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
        ])
        .spacing(8)
        .padding(12);

        panel(column(vec![tab_row.into(), content.into()]).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Ticking
            time::every(Duration::from_millis(30)).map(|_| Message::Tick),
            // Iced events (maybe will be added?)
            event::listen_with(|event, _status, _id| {
                if let Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                }) = event
                {
                    Some(Message::KeyPressed {
                        key,
                        modifiers,
                        text: text.map(|s| s.to_string()),
                    })
                } else {
                    None
                }
            }),
        ])
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
