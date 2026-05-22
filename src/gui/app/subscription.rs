use super::{App, Message};
use iced::advanced::input_method;
use iced::futures::StreamExt;
use iced::futures::channel::mpsc;
use iced::futures::sink::SinkExt;
use iced::stream;
use iced::time::Instant;
use iced::{Event, Subscription, event, keyboard, mouse, time, window};

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        let now = Instant::now();
        let bell_flashing = self
            .bell_flash_start
            .is_some_and(|start| start.elapsed() < super::BELL_FLASH_DURATION);
        let has_animation = self.shell_picker_anim.is_animating(now)
            || self.tabs.iter().any(|tab| tab.sftp.anim.is_animating(now))
            || self.settings_category_transition.is_animating(now)
            || bell_flashing;

        let animation_tick = if has_animation {
            time::every(std::time::Duration::from_millis(16)).map(|_| Message::AnimationTick)
        } else {
            Subscription::none()
        };

        let cursor_blink = if self.config.terminal.cursor_blink
            && self.active_tab != super::SETTINGS_TAB_INDEX
            && self.tabs.get(self.active_tab).is_some()
        {
            time::every(std::time::Duration::from_millis(530)).map(|_| Message::CursorBlink)
        } else {
            Subscription::none()
        };

        let selection_autoscroll = if self.selection_autoscroll.is_some() {
            time::every(std::time::Duration::from_millis(30))
                .map(|_| Message::SelectionAutoscrollTick)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            animation_tick,
            cursor_blink,
            selection_autoscroll,
            Subscription::run(|| {
                stream::channel(100, async |mut output| {
                    let (sender, mut receiver) = mpsc::unbounded();
                    let _ = output.send(Message::PtySenderReady(sender)).await;

                    while let Some(first) = receiver.next().await {
                        let mut batch = vec![first];
                        while let Ok(event) = receiver.try_recv() {
                            batch.push(event);
                        }
                        if batch.len() == 1 {
                            if output
                                .send(Message::PtyOutput(batch.pop().unwrap()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                        } else if output.send(Message::PtyOutputBatch(batch)).await.is_err() {
                            break;
                        }
                    }
                })
            }),
            event::listen_with(|event, status, _id| match event {
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
                Event::InputMethod(input_method::Event::Opened) => {
                    Some(Message::ImeStateChanged(true))
                }
                Event::InputMethod(input_method::Event::Closed) => {
                    Some(Message::ImeStateChanged(false))
                }
                Event::InputMethod(input_method::Event::Commit(text)) => {
                    Some(Message::ImeCommit(text))
                }
                Event::InputMethod(input_method::Event::Preedit(text, cursor)) => {
                    Some(Message::ImePreedit(text, cursor))
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::TabDragRelease)
                }
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::CursorMoved(position))
                }
                Event::Mouse(mouse::Event::WheelScrolled { delta })
                    if !matches!(status, event::Status::Captured) =>
                {
                    let (lines_y, pixels_x) = match delta {
                        mouse::ScrollDelta::Lines { x, y } => (y, x * 30.0),
                        mouse::ScrollDelta::Pixels { x, y } => (y / 20.0, x),
                    };
                    if lines_y.abs() > 0.01 {
                        Some(Message::TerminalWheelScroll(lines_y))
                    } else if pixels_x.abs() > 0.1 {
                        Some(Message::TabBarScroll(pixels_x))
                    } else {
                        None
                    }
                }
                _ => None,
            }),
        ])
    }
}
