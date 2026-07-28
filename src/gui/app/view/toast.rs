use super::super::{App, Message};
use crate::gui::theme::{RADIUS_NORMAL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{button, column, container, row, stack, text};
use iced::{Background, Border, Color, Element, Length, Shadow};

const TOAST_WIDTH: f32 = 320.0;

impl App {
    pub(in crate::gui) fn with_toasts<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let palette = self.palette;

        let cards: Vec<Element<Message>> = self
            .toasts
            .iter()
            .enumerate()
            .map(|(index, toast)| {
                container(
                    row![
                        text(toast.message.as_str())
                            .size(12)
                            .color(palette.text)
                            .width(Length::Fill),
                        button(text("\u{2715}").size(11).color(palette.text_secondary))
                            .style(|_theme: &iced::Theme, _status| {
                                iced::widget::button::Style {
                                    background: Some(Background::Color(Color::TRANSPARENT)),
                                    text_color: Color::WHITE,
                                    border: Border::default(),
                                    shadow: Shadow::default(),
                                    snap: false,
                                }
                            })
                            .padding(0)
                            .on_press(Message::DismissToast(index)),
                    ]
                    .align_y(iced::Alignment::Center)
                    .spacing(SPACING_SMALL),
                )
                .width(Length::Fixed(TOAST_WIDTH))
                .padding([10, 12])
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(palette.surface)),
                    border: Border {
                        radius: RADIUS_NORMAL.into(),
                        width: 1.0,
                        color: Color {
                            a: 0.15,
                            ..palette.text
                        },
                    },
                    shadow: Shadow {
                        color: Color {
                            a: 0.35,
                            ..Color::BLACK
                        },
                        offset: iced::Vector::new(0.0, 4.0),
                        blur_radius: 16.0,
                    },
                    ..Default::default()
                })
                .into()
            })
            .collect();

        let overlay = container(column(cards).spacing(SPACING_SMALL))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(SPACING_NORMAL);

        stack![base_layout.into(), overlay].into()
    }
}
