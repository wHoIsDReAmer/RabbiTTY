use crate::gui::app::Message;
use crate::gui::theme::Palette;
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

    // macOS: 좌측에 트래픽 라이트(닫기/최소화/확대) 공간 확보
    #[cfg(target_os = "macos")]
    let left_padding = 80.0;
    #[cfg(not(target_os = "macos"))]
    let left_padding = 8.0;

    let padding = iced::Padding::new(6.0).left(left_padding).right(8.0);

    container(
        row(tab_elements)
            .spacing(2)
            .align_y(iced::Alignment::Center),
    )
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        ..Default::default()
    })
    .padding(padding)
    .width(Length::Fill)
    .into()
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
                border: Border {
                    radius: 8.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
            }
        });

    tab_button.into()
}
