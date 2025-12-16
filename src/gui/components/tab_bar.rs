use crate::gui::app::Message;
use crate::gui::theme::Palette;
#[cfg(target_os = "windows")]
use iced::widget::mouse_area;
use iced::widget::{button, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};

pub fn tab_bar<'a>(
    tabs: impl Iterator<Item = (&'a str, usize, bool)>, // (title, index, is_active)
    on_add: Message,
) -> Element<'a, Message> {
    let palette = Palette::DARK;

    let mut tab_elements: Vec<Element<Message>> = Vec::new();

    for (title, index, is_active) in tabs {
        let tab_item = browser_tab(title, index, is_active);
        tab_elements.push(tab_item);
    }

    let add_btn = button(text("+").size(14))
        .on_press(on_add)
        .padding([4, 7])
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: match status {
                    button::Status::Hovered => palette.text,
                    _ => palette.text_secondary,
                },
                border: Border {
                    radius: 6.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
            },
        );

    tab_elements.push(add_btn.into());

    // macOS: left control buttons
    #[cfg(target_os = "macos")]
    let left_padding = 80.0;
    #[cfg(not(target_os = "macos"))]
    let left_padding = 0.0;

    let padding = iced::Padding::new(0.0).left(left_padding);

    // Windows: right control buttons
    #[cfg(target_os = "windows")]
    let window_controls = {
        let minimize_btn = button(text("─").size(12))
            .on_press(Message::WindowMinimize)
            .padding([6, 12])
            .style(
                move |_theme: &Theme, status: button::Status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(Color {
                            a: 0.2,
                            ..palette.text
                        })),
                        _ => Some(Background::Color(Color::TRANSPARENT)),
                    },
                    text_color: palette.text_secondary,
                    border: Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: iced::Shadow::default(),
                },
            );

        let maximize_btn = button(text("□").size(12))
            .on_press(Message::WindowMaximize)
            .padding([6, 12])
            .style(
                move |_theme: &Theme, status: button::Status| button::Style {
                    background: match status {
                        button::Status::Hovered => Some(Background::Color(Color {
                            a: 0.2,
                            ..palette.text
                        })),
                        _ => Some(Background::Color(Color::TRANSPARENT)),
                    },
                    text_color: palette.text_secondary,
                    border: Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: iced::Shadow::default(),
                },
            );

        let close_btn = button(text("✕").size(12))
            .on_press(Message::Exit)
            .padding([6, 12])
            .style(
                move |_theme: &Theme, status: button::Status| button::Style {
                    background: match status {
                        button::Status::Hovered => {
                            Some(Background::Color(Color::from_rgb(0.9, 0.2, 0.2)))
                        }
                        _ => Some(Background::Color(Color::TRANSPARENT)),
                    },
                    text_color: match status {
                        button::Status::Hovered => Color::WHITE,
                        _ => palette.text_secondary,
                    },
                    border: Border {
                        radius: 0.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: iced::Shadow::default(),
                },
            );

        row![minimize_btn, maximize_btn, close_btn].spacing(0)
    };

    #[cfg(target_os = "windows")]
    let content = {
        let spacer = iced::widget::horizontal_space();
        row(tab_elements)
            .push(spacer)
            .push(window_controls)
            .spacing(2)
            .align_y(iced::Alignment::Center)
    };

    #[cfg(not(target_os = "windows"))]
    let content = row(tab_elements)
        .spacing(2)
        .align_y(iced::Alignment::Center);

    let tab_bar_container = container(content)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            ..Default::default()
        })
        .padding(padding)
        .width(Length::Fill);

    // Windows: Enable window dragging by clicking on the tab bar background
    #[cfg(target_os = "windows")]
    return mouse_area(tab_bar_container)
        .on_press(Message::WindowDrag)
        .into();

    #[cfg(not(target_os = "windows"))]
    tab_bar_container.into()
}

fn browser_tab<'a>(title: &'a str, index: usize, is_active: bool) -> Element<'a, Message> {
    let palette = Palette::DARK;

    let tab_text = text(title).size(13);

    let close_btn = button(text("✕").size(10))
        .on_press(Message::CloseTab(index))
        .padding([2, 4])
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(Color {
                        a: 0.2,
                        ..palette.text
                    })),
                    _ => Some(Background::Color(Color::TRANSPARENT)),
                },
                text_color: palette.text_secondary,
                border: Border {
                    radius: 4.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
            },
        );

    let tab_content = row![tab_text, close_btn]
        .spacing(8)
        .align_y(iced::Alignment::Center);

    let tab_button = button(tab_content)
        .on_press(Message::TabSelected(index))
        .padding([8, 12])
        .style(move |_theme: &Theme, status: button::Status| {
            let (bg_color, text_color) = if is_active {
                (palette.background, palette.text)
            } else {
                let hover_bg = match status {
                    button::Status::Hovered => Color {
                        a: 0.3,
                        ..palette.background
                    },
                    _ => Color::TRANSPARENT,
                };
                (hover_bg, palette.text_secondary)
            };

            button::Style {
                background: Some(Background::Color(bg_color)),
                text_color,
                shadow: iced::Shadow::default(),
                ..Default::default()
            }
        });

    tab_button.into()
}
