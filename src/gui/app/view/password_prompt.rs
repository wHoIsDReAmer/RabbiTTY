//! Modal dialog that prompts for an SSH password when an SSH profile lacks
//! one in both its config and the keychain.

use super::super::{Message, PasswordPromptState};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{
    button, center, checkbox, column, container, mouse_area, row, stack, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Shadow};

pub(in crate::gui) fn password_prompt<'a>(
    base_layout: impl Into<Element<'a, Message>>,
    state: &'a PasswordPromptState,
    palette: Palette,
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
    .on_press(Message::SshPasswordPromptCancel);

    let label = format!(
        "Password for {}@{}",
        if state.profile.user.is_empty() {
            "<user>"
        } else {
            state.profile.user.as_str()
        },
        state.profile.host
    );

    let input = text_input("password", &state.draft)
        .secure(true)
        .on_input(Message::SshPasswordPromptChanged)
        .on_submit(Message::SshPasswordPromptSubmit)
        .padding([6, 10])
        .size(13)
        .width(Length::Fill);

    let save_toggle = checkbox(state.save_to_keychain)
        .label("Save to keychain")
        .on_toggle(Message::SshPasswordPromptToggleSave)
        .size(14)
        .text_size(12);

    let error: Element<Message> = match state.error.as_deref() {
        Some(msg) => text(msg).size(12).color(palette.error).into(),
        None => container("").into(),
    };

    let cancel_btn = button(text("Cancel").size(13))
        .style(secondary_button_style(palette))
        .padding([6, 14])
        .on_press(Message::SshPasswordPromptCancel);
    let connect_btn = button(text("Connect").size(13))
        .style(primary_button_style(palette))
        .padding([6, 14])
        .on_press(Message::SshPasswordPromptSubmit);

    let popup_card = container(
        column(vec![
            text(label).size(15).into(),
            input.into(),
            save_toggle.into(),
            error,
            row![cancel_btn, connect_btn].spacing(SPACING_SMALL).into(),
        ])
        .spacing(SPACING_NORMAL)
        .padding(20)
        .width(Length::Fixed(320.0)),
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

    stack![
        base_layout.into(),
        backdrop,
        center(popup_card).width(Length::Fill).height(Length::Fill),
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn primary_button_style(
    palette: Palette,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme: &iced::Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(Background::Color(if hovered {
                Color {
                    a: 0.9,
                    ..palette.accent
                }
            } else {
                palette.accent
            })),
            text_color: palette.background,
            border: Border {
                radius: RADIUS_SMALL.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}

fn secondary_button_style(
    palette: Palette,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme: &iced::Theme, status: button::Status| {
        let hovered = matches!(status, button::Status::Hovered);
        button::Style {
            background: Some(Background::Color(Color {
                a: if hovered { 0.15 } else { 0.08 },
                ..palette.text
            })),
            text_color: palette.text,
            border: Border {
                radius: RADIUS_SMALL.into(),
                ..Default::default()
            },
            shadow: Shadow::default(),
            snap: true,
        }
    }
}
