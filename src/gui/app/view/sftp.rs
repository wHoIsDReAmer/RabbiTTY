//! SFTP drawer rendering.

use crate::gui::app::{Message, SftpMessage};
use crate::gui::components::{HoverStyle, hover_fade};
use crate::gui::sftp::{self, SftpDrawerState, TransferRow};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use crate::ssh::sftp::Entry;
use iced::widget::{Space, button, column, container, progress_bar, row, scrollable, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Shadow, Theme, Vector};

const ROW_HEIGHT: f32 = 24.0;
const HEADER_HEIGHT: f32 = 32.0;

pub fn drawer<'a>(
    state: &'a SftpDrawerState,
    tab_id: u64,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let header = drawer_header(state, palette, animations_enabled);
    let separator = container("")
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..palette.text
            })),
            ..Default::default()
        });
    let body = drawer_body(state, tab_id, palette, animations_enabled);

    let mut layers: Vec<Element<Message>> = vec![header, separator.into(), body];
    if !state.transfers.is_empty() {
        layers.push(transfer_strip(state, palette, animations_enabled));
    }

    container(column(layers).width(Length::Fill).height(Length::Fill))
        .padding([SPACING_SMALL, SPACING_NORMAL])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.98,
                ..palette.surface
            })),
            border: Border {
                radius: iced::border::Radius {
                    top_left: RADIUS_NORMAL,
                    top_right: RADIUS_NORMAL,
                    bottom_right: 0.0,
                    bottom_left: 0.0,
                },
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow {
                color: Color {
                    a: 0.3,
                    ..Color::BLACK
                },
                offset: Vector::new(0.0, -2.0),
                blur_radius: 12.0,
            },
            ..Default::default()
        })
        .into()
}

fn drawer_header<'a>(
    state: &'a SftpDrawerState,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let path = if state.current_path.is_empty() {
        "~"
    } else {
        state.current_path.as_str()
    };
    let path_label = text(path).size(12).color(Color {
        a: 0.8,
        ..palette.text
    });

    let status = if state.opening {
        Some("opening")
    } else if state.loading {
        Some("loading")
    } else {
        None
    };
    let status_el: Element<Message> = match status {
        Some(s) => text(s)
            .size(11)
            .color(Color {
                a: 0.5,
                ..palette.text
            })
            .into(),
        None => Space::new()
            .width(Length::Shrink)
            .height(Length::Shrink)
            .into(),
    };

    let upload_btn = drawer_icon_button(
        "\u{2191}",
        Message::Sftp(SftpMessage::RequestUpload),
        palette,
        animations_enabled,
    );
    let refresh_btn = drawer_icon_button(
        "\u{27F3}",
        Message::Sftp(SftpMessage::Refresh),
        palette,
        animations_enabled,
    );
    let close_btn = drawer_icon_button(
        "\u{2715}",
        Message::Sftp(SftpMessage::ToggleDrawer),
        palette,
        animations_enabled,
    );

    container(
        row![
            path_label,
            Space::new().width(Length::Fill),
            status_el,
            upload_btn,
            refresh_btn,
            close_btn,
        ]
        .spacing(SPACING_SMALL)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .height(Length::Fixed(HEADER_HEIGHT))
    .center_y(Length::Fixed(HEADER_HEIGHT))
    .into()
}

/// Small icon button used by the SFTP drawer header. Uses a slightly tighter
/// padding than the global `icon` factory so it fits the header height.
fn drawer_icon_button<'a>(
    glyph: &'a str,
    on_press: Message,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let inner = button(text(glyph.to_string()).size(12))
        .on_press(on_press)
        .padding([3, 8])
        .style(
            move |_theme: &Theme, _status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: Color {
                    a: 0.7,
                    ..palette.text
                },
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: Shadow::default(),
                snap: true,
            },
        );
    let rest = HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: RADIUS_SMALL,
    };
    let hover = HoverStyle {
        background: Color {
            a: 0.08,
            ..palette.text
        },
        ..rest
    };
    hover_fade(inner, rest, hover, animations_enabled).into()
}

fn drawer_body<'a>(
    state: &'a SftpDrawerState,
    tab_id: u64,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    if let Some(error) = state.error.as_deref() {
        return centered_message(error, palette.error, palette);
    }
    if state.opening || (state.loading && state.entries.is_empty()) {
        return centered_message(
            "Loading…",
            Color {
                a: 0.5,
                ..palette.text
            },
            palette,
        );
    }

    let parent_element = sftp::parent_path(&state.current_path)
        .map(|p| parent_row(tab_id, p, palette, animations_enabled));

    if state.entries.is_empty() {
        let empty = centered_message(
            "Empty directory",
            Color {
                a: 0.5,
                ..palette.text
            },
            palette,
        );
        return match parent_element {
            Some(parent) => column(vec![parent, empty])
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
            None => empty,
        };
    }

    let mut rows: Vec<Element<Message>> = Vec::with_capacity(state.entries.len() + 1);
    if let Some(parent) = parent_element {
        rows.push(parent);
    }
    for entry in &state.entries {
        rows.push(entry_row(state, tab_id, entry, palette, animations_enabled));
    }

    scrollable(column(rows).width(Length::Fill).padding(iced::Padding {
        top: 4.0,
        right: 12.0,
        bottom: 4.0,
        left: 0.0,
    }))
    .style(crate::gui::theme::scrollbar_style(palette))
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn centered_message<'a>(msg: &'a str, color: Color, _palette: Palette) -> Element<'a, Message> {
    container(text(msg).size(12).color(color))
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn parent_row<'a>(
    tab_id: u64,
    parent: String,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let row_style = row_style_factory(palette);
    let inner = button(
        row![
            text("\u{2190}").size(11).color(Color {
                a: 0.55,
                ..palette.text
            }),
            text("..").size(12).color(Color {
                a: 0.7,
                ..palette.text
            }),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Sftp(SftpMessage::Navigate {
        tab_id,
        path: parent,
    }))
    .padding([3.0, SPACING_NORMAL])
    .width(Length::Fill)
    .height(Length::Fixed(ROW_HEIGHT))
    .style(row_style);
    hover_fade(
        inner,
        row_rest_style(),
        row_hover_style(palette),
        animations_enabled,
    )
    .into()
}

fn entry_row<'a>(
    state: &'a SftpDrawerState,
    tab_id: u64,
    entry: &'a Entry,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let marker = if entry.is_dir {
        "\u{25B8}"
    } else if entry.is_symlink {
        "\u{2937}"
    } else {
        " "
    };
    let marker_color = if entry.is_dir {
        Color {
            a: 0.6,
            ..palette.text
        }
    } else {
        Color {
            a: 0.35,
            ..palette.text
        }
    };
    let name_color = Color {
        a: if entry.is_dir { 0.95 } else { 0.85 },
        ..palette.text
    };
    let size_text = if entry.is_dir {
        String::new()
    } else {
        humanize_bytes(entry.size)
    };

    let name_with_suffix = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let row_content = row![
        text(marker).size(11).color(marker_color),
        text(name_with_suffix).size(12).color(name_color),
        Space::new().width(Length::Fill),
        text(size_text).size(11).color(Color {
            a: 0.5,
            ..palette.text
        }),
    ]
    .spacing(SPACING_NORMAL)
    .align_y(Alignment::Center);

    let press_msg = if entry.is_dir {
        Some(Message::Sftp(SftpMessage::Navigate {
            tab_id,
            path: sftp::join_path(&state.current_path, &entry.name),
        }))
    } else if !entry.is_symlink {
        Some(Message::Sftp(SftpMessage::RequestDownload {
            tab_id,
            remote: sftp::join_path(&state.current_path, &entry.name),
            suggested_name: entry.name.clone(),
        }))
    } else {
        None
    };

    let row_style = row_style_factory(palette);

    let mut btn = button(row_content)
        .padding([3.0, SPACING_NORMAL])
        .width(Length::Fill)
        .height(Length::Fixed(ROW_HEIGHT))
        .style(row_style);
    if let Some(msg) = press_msg {
        btn = btn.on_press(msg);
    }
    hover_fade(
        btn,
        row_rest_style(),
        row_hover_style(palette),
        animations_enabled,
    )
    .into()
}

fn row_rest_style() -> HoverStyle {
    HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: RADIUS_SMALL,
    }
}

fn row_hover_style(palette: Palette) -> HoverStyle {
    HoverStyle {
        background: Color {
            a: 0.06,
            ..palette.text
        },
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: RADIUS_SMALL,
    }
}

fn row_style_factory(palette: Palette) -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    // Background is painted by `hover_fade` behind the button; the button
    // itself stays transparent so the cross-fade can show through.
    move |_theme: &Theme, _status: button::Status| button::Style {
        background: Some(Background::Color(Color::TRANSPARENT)),
        text_color: palette.text,
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: Shadow::default(),
        snap: true,
    }
}

fn transfer_strip<'a>(
    state: &'a SftpDrawerState,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let separator = container("")
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: 0.08,
                ..palette.text
            })),
            ..Default::default()
        });

    let rows: Vec<Element<Message>> = state
        .transfers
        .iter()
        .map(|row| transfer_row(row, palette, animations_enabled))
        .collect();

    column![
        separator,
        column(rows)
            .spacing(2)
            .padding([4.0, 0.0])
            .width(Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}

fn transfer_row<'a>(
    row_state: &'a TransferRow,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let name = row_state.path.rsplit('/').next().unwrap_or(&row_state.path);
    let progress = if row_state.total > 0 {
        (row_state.transferred as f32 / row_state.total as f32).clamp(0.0, 1.0)
    } else if row_state.finished {
        1.0
    } else {
        0.0
    };
    let percent = (progress * 100.0).round() as u32;
    let bytes_text = if row_state.total > 0 {
        format!(
            "{} / {}",
            humanize_bytes(row_state.transferred),
            humanize_bytes(row_state.total)
        )
    } else {
        humanize_bytes(row_state.transferred)
    };

    let bar = progress_bar(0.0..=1.0, progress)
        .girth(Length::Fixed(4.0))
        .style(move |_theme: &Theme| iced::widget::progress_bar::Style {
            background: Background::Color(Color {
                a: 0.12,
                ..palette.text
            }),
            bar: Background::Color(palette.accent),
            border: Border {
                radius: 2.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
        });

    let status_marker: Element<Message> = if row_state.finished {
        text("\u{2713}")
            .size(12)
            .color(Color {
                a: 0.7,
                ..palette.accent
            })
            .into()
    } else {
        let cancel_style = move |_theme: &Theme, _status: button::Status| button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: Color {
                a: 0.55,
                ..palette.text
            },
            border: Border {
                radius: RADIUS_SMALL.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            shadow: Shadow::default(),
            snap: true,
        };
        let cancel_btn = button(text("\u{2715}").size(10))
            .on_press(Message::Sftp(SftpMessage::CancelTransfer))
            .padding([2, 6])
            .style(cancel_style);
        let cancel_rest = HoverStyle {
            background: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: RADIUS_SMALL,
        };
        let cancel_hover = HoverStyle {
            background: Color {
                a: 0.12,
                ..palette.text
            },
            ..cancel_rest
        };
        hover_fade(cancel_btn, cancel_rest, cancel_hover, animations_enabled).into()
    };

    container(
        column![
            row![
                text(name).size(11).color(Color {
                    a: 0.85,
                    ..palette.text
                }),
                Space::new().width(Length::Fill),
                text(format!("{percent}%")).size(11).color(Color {
                    a: 0.6,
                    ..palette.text
                }),
                text(bytes_text).size(11).color(Color {
                    a: 0.45,
                    ..palette.text
                }),
                status_marker,
            ]
            .spacing(SPACING_SMALL)
            .align_y(Alignment::Center),
            bar,
        ]
        .spacing(3),
    )
    .padding([4.0, SPACING_SMALL])
    .width(Length::Fill)
    .into()
}

fn humanize_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[unit])
    }
}
