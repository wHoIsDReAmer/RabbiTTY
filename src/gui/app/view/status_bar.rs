use super::super::{App, Message};
use crate::gui::theme::{RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{button, container, row, text, tooltip};
use iced::{Background, Border, Color, Element, Length};

const BAR_HEIGHT: f32 = 22.0;

impl App {
    pub(in crate::gui) fn view_status_bar(&self) -> Option<Element<'_, Message>> {
        let items = self.plugins.as_ref()?.status_items();
        if items.is_empty() {
            return None;
        }

        let palette = self.palette;
        let cells: Vec<Element<Message>> = items
            .into_iter()
            .map(|(plugin, item)| cell(plugin, item, palette))
            .collect();

        Some(
            container(
                row(cells)
                    .spacing(SPACING_NORMAL)
                    .align_y(iced::Alignment::Center),
            )
            .height(Length::Fixed(BAR_HEIGHT))
            .width(Length::Fill)
            .padding([0, SPACING_NORMAL as u16])
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_theme: &iced::Theme| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.22,
                    ..palette.surface
                })),
                ..Default::default()
            })
            .into(),
        )
    }
}

fn cell<'a>(
    plugin: String,
    item: crate::plugin::StatusItem,
    palette: crate::gui::theme::Palette,
) -> Element<'a, Message> {
    let label = text(item.text).size(11).color(palette.text_secondary);

    let body: Element<Message> = match item.command {
        Some(command) => button(label)
            .style(move |_theme: &iced::Theme, status| {
                let hovered = matches!(status, button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(if hovered {
                        Color {
                            a: 0.10,
                            ..palette.text
                        }
                    } else {
                        Color::TRANSPARENT
                    })),
                    text_color: palette.text,
                    border: Border {
                        radius: RADIUS_SMALL.into(),
                        ..Default::default()
                    },
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            })
            .padding([1, SPACING_SMALL as u16])
            .on_press(Message::RunPluginCommand { plugin, command })
            .into(),
        None => label.into(),
    };

    match item.tooltip {
        Some(hint) => tooltip(
            body,
            container(text(hint).size(11).color(palette.text))
                .padding([4, 8])
                .style(move |_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(palette.surface)),
                    border: Border {
                        radius: RADIUS_SMALL.into(),
                        width: 1.0,
                        color: Color {
                            a: 0.15,
                            ..palette.text
                        },
                    },
                    ..Default::default()
                }),
            tooltip::Position::Top,
        )
        .into(),
        None => body,
    }
}
