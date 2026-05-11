use super::super::{App, Message};
use crate::gui::settings::{self, SettingsCategory};
use crate::gui::theme::{RADIUS_NORMAL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length};

impl App {
    pub(in crate::gui) fn view_settings(&self) -> Element<'_, Message> {
        let palette = self.palette;
        let mut category_items: Vec<Element<Message>> = Vec::new();

        for category in SettingsCategory::ALL {
            let is_active = category == self.settings_category;
            let icon = category.icon();
            let label = category.label();
            let button_style = move |_theme: &iced::Theme, status: iced::widget::button::Status| {
                let bg = if is_active {
                    Color {
                        a: 0.12,
                        ..palette.text
                    }
                } else {
                    match status {
                        iced::widget::button::Status::Hovered => Color {
                            a: 0.08,
                            ..palette.text
                        },
                        _ => Color::TRANSPARENT,
                    }
                };

                iced::widget::button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: if is_active {
                        palette.text
                    } else {
                        palette.text_secondary
                    },
                    border: Border {
                        radius: RADIUS_NORMAL.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            };

            let btn_content = row![text(icon).size(14), text(label).size(13),]
                .spacing(SPACING_SMALL)
                .align_y(iced::Alignment::Center);

            let item = button(btn_content)
                .padding([8, 12])
                .width(Length::Fill)
                .on_press(Message::SelectSettingsCategory(category))
                .style(button_style);
            category_items.push(item.into());
        }

        let sidebar = container(
            column(category_items)
                .spacing(SPACING_SMALL)
                .padding(SPACING_NORMAL)
                .width(Length::Fill),
        )
        .width(Length::Fixed(180.0))
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(palette.surface)),
            ..Default::default()
        });

        let breadcrumb = row![
            text("Settings").size(18),
            text("/").size(16).color(Color {
                a: 0.3,
                ..palette.text
            }),
            text(self.settings_category.label())
                .size(16)
                .color(palette.text),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(SPACING_SMALL);

        let header: Element<Message> = row![breadcrumb, container("").width(Length::Fill),]
            .align_y(iced::Alignment::Center)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into();

        let body_content = container(settings::view_category(
            self.settings_category,
            &self.config,
            &self.settings_draft,
            &self.font_combo_state,
            self.show_all_fonts,
            &self.all_font_options,
            palette,
        ))
        .padding([0, 12])
        .width(Length::Fill);

        let body = scrollable(body_content)
            .height(Length::Fill)
            .width(Length::Fill);

        let content = container(
            column(vec![header, body.into()])
                .spacing(SPACING_NORMAL)
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(SPACING_LARGE);

        let settings_layout: Element<Message> = row![sidebar, content]
            .spacing(SPACING_LARGE)
            .height(Length::Fill)
            .width(Length::Fill)
            .into();

        if matches!(self.settings_category, SettingsCategory::Ssh) {
            settings::ssh::modal_overlay(settings_layout, &self.settings_draft, palette)
        } else {
            settings_layout
        }
    }
}
