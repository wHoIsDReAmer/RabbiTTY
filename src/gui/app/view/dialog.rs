use super::super::Message;
use crate::gui::components::{primary, secondary};
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{center, column, container, mouse_area, row, stack, text};
use iced::{Background, Border, Color, Element, Length};

pub(in crate::gui) struct DialogButton {
    pub label: String,
    pub message: Message,
    pub primary: bool,
}

#[allow(clippy::too_many_arguments)]
pub(in crate::gui) fn confirm_dialog<'a>(
    base_layout: impl Into<Element<'a, Message>>,
    title: &str,
    description: &str,
    buttons: Vec<DialogButton>,
    on_dismiss: Message,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let backdrop = mouse_area(
        container(text(""))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                })),
                ..Default::default()
            }),
    )
    .on_press(on_dismiss);

    let button_row: Vec<Element<Message>> = buttons
        .into_iter()
        .map(|btn| {
            if btn.primary {
                primary(btn.label, btn.message, palette, animations_enabled)
            } else {
                secondary(btn.label, Some(btn.message), palette, animations_enabled)
            }
        })
        .collect();

    let popup_card = container(
        column(vec![
            text(title.to_string()).size(16).into(),
            text(description.to_string()).size(13).into(),
            row(button_row).spacing(SPACING_SMALL).into(),
        ])
        .spacing(SPACING_NORMAL)
        .padding(20)
        .width(Length::Fixed(300.0)),
    )
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            radius: (RADIUS_NORMAL + 4.0).into(),
            width: 1.0,
            color: Color {
                a: 0.15,
                ..palette.text
            },
        },
        ..Default::default()
    });

    let centered_popup = center(popup_card).width(Length::Fill).height(Length::Fill);

    stack![base_layout.into(), backdrop, centered_popup]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
