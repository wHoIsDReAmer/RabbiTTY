//! SFTP drawer rendering. Phase 2 placeholder — file list, transfers, and
//! mutation controls land in later phases.

use crate::gui::app::Message;
use crate::gui::sftp::SftpDrawerState;
use crate::gui::theme::{Palette, RADIUS_NORMAL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

const DRAWER_TOP_BORDER: f32 = 1.0;

pub fn drawer<'a>(state: &'a SftpDrawerState, palette: Palette) -> Element<'a, Message> {
    let header = drawer_header(state, palette);
    let body = drawer_body(state, palette);

    container(
        column(vec![header, body])
            .spacing(SPACING_SMALL)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .padding([SPACING_NORMAL, SPACING_NORMAL])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_theme: &Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.96,
            ..palette.surface
        })),
        border: Border {
            radius: 0.0.into(),
            width: DRAWER_TOP_BORDER,
            color: Color {
                a: 0.2,
                ..palette.text
            },
        },
        ..Default::default()
    })
    .into()
}

fn drawer_header<'a>(state: &'a SftpDrawerState, palette: Palette) -> Element<'a, Message> {
    let path_label = text(if state.current_path.is_empty() {
        "."
    } else {
        state.current_path.as_str()
    })
    .size(13)
    .color(palette.text);

    let status = if state.opening {
        Some("Opening…")
    } else if state.loading {
        Some("Loading…")
    } else {
        None
    };
    let status_text: Element<Message> = match status {
        Some(s) => text(s).size(12).color(palette.text_secondary).into(),
        None => Space::new()
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into(),
    };

    let close_style = move |_theme: &Theme, status: button::Status| button::Style {
        background: Some(Background::Color(match status {
            button::Status::Hovered => Color {
                a: 0.12,
                ..palette.text
            },
            _ => Color::TRANSPARENT,
        })),
        text_color: palette.text_secondary,
        border: Border {
            radius: RADIUS_NORMAL.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: iced::Shadow::default(),
        snap: true,
    };
    let close_btn = button(text("\u{2715}").size(12))
        .on_press(Message::SftpToggleDrawer)
        .padding([4, 10])
        .style(close_style);

    row![
        text("SFTP").size(14).color(palette.accent),
        path_label,
        Space::new().width(Length::Fill),
        status_text,
        close_btn,
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn drawer_body<'a>(state: &'a SftpDrawerState, palette: Palette) -> Element<'a, Message> {
    if let Some(error) = state.error.as_deref() {
        return container(text(error).size(12).color(palette.error))
            .padding([SPACING_NORMAL, 0.0])
            .into();
    }

    container(
        text("File browser coming online…")
            .size(12)
            .color(palette.text_secondary),
    )
    .padding([SPACING_NORMAL, 0.0])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
