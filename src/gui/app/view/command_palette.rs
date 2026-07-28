use super::super::update::palette::COMMAND_INPUT_ID;
use super::super::{App, Message};
use crate::gui::components::{HoverStyle, hover_fade};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{
    button, column, container, mouse_area, row, scrollable, stack, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Shadow};

const PALETTE_WIDTH: f32 = 560.0;
const LIST_HEIGHT: f32 = 320.0;

impl App {
    pub(in crate::gui) fn view_command_palette<'a>(
        &'a self,
        base_layout: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let palette = self.palette;

        let backdrop = mouse_area(
            container(text(""))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme: &iced::Theme| container::Style {
                    background: Some(Background::Color(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.45,
                    })),
                    ..Default::default()
                }),
        )
        .on_press(Message::CloseCommandPalette);

        let input = text_input(t!("command_palette.placeholder"), &self.command_query)
            .id(COMMAND_INPUT_ID.clone())
            .on_input(Message::CommandQueryChanged)
            .padding([10, 14])
            .size(15)
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme, _status| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                icon: palette.text_secondary,
                placeholder: palette.text_secondary,
                value: palette.text,
                selection: Color {
                    a: 0.3,
                    ..palette.accent
                },
            });

        let entries = self.visible_command_entries();
        let body: Element<Message> = if entries.is_empty() {
            container(
                text(t!("command_palette.no_matches"))
                    .size(13)
                    .color(palette.text_secondary),
            )
            .padding(SPACING_NORMAL)
            .into()
        } else {
            let rows: Vec<Element<Message>> = entries
                .iter()
                .enumerate()
                .map(|(index, entry)| {
                    entry_row(
                        index,
                        &entry.label,
                        &entry.detail,
                        index == self.command_selected,
                        palette,
                        self.config.ui.animations_enabled,
                    )
                })
                .collect();

            scrollable(
                column(rows)
                    .spacing(2)
                    .padding([SPACING_SMALL, SPACING_SMALL]),
            )
            .height(Length::Fixed(LIST_HEIGHT))
            .into()
        };

        let card = container(column![input, divider(palette), body].spacing(0))
            .width(Length::Fixed(PALETTE_WIDTH))
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
                    offset: iced::Vector::new(0.0, 8.0),
                    blur_radius: 24.0,
                },
                ..Default::default()
            });

        let positioned = container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .padding([90, 0]);

        stack![base_layout.into(), backdrop, positioned].into()
    }
}

fn divider<'a>(palette: Palette) -> Element<'a, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(1)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.1,
                ..palette.text
            })),
            ..Default::default()
        })
        .into()
}

fn entry_row<'a>(
    index: usize,
    label: &str,
    detail: &str,
    selected: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let content = row![
        text(label.to_string()).size(13).color(palette.text),
        iced::widget::Space::new().width(Length::Fill),
        text(detail.to_string())
            .size(11)
            .color(palette.text_secondary),
    ]
    .align_y(iced::Alignment::Center)
    .spacing(SPACING_NORMAL);

    let inner = button(content)
        .style(
            move |_theme: &iced::Theme, _status: iced::widget::button::Status| {
                iced::widget::button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: palette.text,
                    border: Border {
                        radius: RADIUS_SMALL.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: Shadow::default(),
                    snap: false,
                }
            },
        )
        .padding([7, 10])
        .width(Length::Fill)
        .on_press(Message::RunCommandEntry(index));

    let rest = HoverStyle {
        background: if selected {
            Color {
                a: 0.16,
                ..palette.accent
            }
        } else {
            Color::TRANSPARENT
        },
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: RADIUS_SMALL,
    };
    let hover = if selected {
        rest
    } else {
        HoverStyle {
            background: Color {
                a: 0.08,
                ..palette.text
            },
            ..rest
        }
    };

    hover_fade(inner, rest, hover, animations_enabled).into()
}
