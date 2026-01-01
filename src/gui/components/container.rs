use iced::widget::container;
use iced::{Background, Color, Theme};

pub fn panel<'a>(
    content: impl Into<iced::Element<'a, crate::gui::app::Message>>,
    background: Option<Color>,
    text_color: Color,
) -> container::Container<'a, crate::gui::app::Message> {
    container(content).style(move |_theme: &Theme| container::Style {
        background: background.map(Background::Color),
        text_color: Some(text_color),
        ..Default::default()
    })
}
