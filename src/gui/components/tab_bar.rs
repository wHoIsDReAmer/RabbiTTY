use crate::gui::app::Message;
use crate::gui::components::{HoverStyle, button as button_factory, hover_fade};
use crate::gui::theme::Palette;
use iced::widget::mouse_area;
use iced::widget::{button, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

#[allow(clippy::too_many_arguments)]
pub fn tab_bar<'a>(
    tabs: impl Iterator<Item = (&'a str, usize, bool)>,
    on_add: Message,
    on_settings: Message,
    sftp_toggle: Option<(Message, bool)>,
    bar_alpha: f32,
    tab_alpha: f32,
    dragging_tab: Option<usize>,
    drag_target: Option<usize>,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let mut tab_elements: Vec<Element<Message>> = Vec::new();
    let is_reordering =
        dragging_tab.is_some() && drag_target.is_some() && dragging_tab != drag_target;

    for (title, index, is_active) in tabs {
        // Insert drop indicator before the target tab
        if is_reordering && drag_target == Some(index) {
            let gap = container(text("")).width(24).height(Length::Shrink).style(
                move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.15,
                        ..palette.text
                    })),
                    border: Border {
                        radius: 4.0.into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                    },
                    ..Default::default()
                },
            );

            tab_elements.push(gap.into());
        }

        let tab_item = browser_tab(
            title,
            index,
            is_active,
            tab_alpha,
            palette,
            animations_enabled,
        );
        let is_terminal_tab = index != crate::gui::app::SETTINGS_TAB_INDEX;
        let mut tab_item = mouse_area(tab_item)
            .on_press(Message::TabSelected(index))
            .on_enter(Message::TabDragHover(index));
        if is_terminal_tab {
            tab_item = tab_item.on_right_press(Message::ShowTabContextMenu(index));
        }
        tab_elements.push(tab_item.into());
    }

    let add_btn = button_factory::icon("+", on_add, palette, animations_enabled);
    let settings_btn = button_factory::icon("\u{2699}", on_settings, palette, animations_enabled);

    let sftp_btn: Option<Element<Message>> = sftp_toggle.map(|(msg, active)| {
        button_factory::icon_toggle("\u{21C5}", msg, active, palette, animations_enabled)
    });

    let tabs_row = row(tab_elements)
        .spacing(2)
        .align_y(iced::Alignment::Center);
    let tabs_scroll = scrollable(tabs_row)
        .id(crate::gui::app::update::TAB_BAR_SCROLLABLE_ID.clone())
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(3).scroller_width(3),
        ))
        .on_scroll(|viewport: scrollable::Viewport| {
            Message::TabBarScrolled(viewport.absolute_offset().x)
        })
        .style(crate::gui::theme::scrollbar_style(palette))
        .width(Length::Fill)
        .height(Length::Shrink);

    // macOS: left control buttons
    #[cfg(target_os = "macos")]
    let left_padding = 80.0;
    #[cfg(not(target_os = "macos"))]
    let left_padding = 0.0;

    let padding = iced::Padding::new(0.0).left(left_padding);

    // Windows: right control buttons
    #[cfg(target_os = "windows")]
    let window_controls = {
        let hover_subtle = Color {
            a: 0.15,
            ..palette.text
        };
        let hover_close = Color::from_rgb(0.9, 0.2, 0.2);

        let win_style = move |hover_color: Color| {
            move |_theme: &Theme, status: button::Status| button::Style {
                background: match status {
                    button::Status::Hovered => Some(Background::Color(hover_color)),
                    _ => Some(Background::Color(Color::TRANSPARENT)),
                },
                text_color: match status {
                    button::Status::Hovered => Color::WHITE,
                    _ => palette.text_secondary,
                },
                border: Border::default(),
                shadow: iced::Shadow::default(),
                snap: true,
            }
        };

        row![
            button(text("\u{2500}").size(12))
                .on_press(Message::WindowMinimize)
                .padding([6, 12])
                .style(win_style(hover_subtle)),
            button(text("\u{25a1}").size(12))
                .on_press(Message::WindowMaximize)
                .padding([6, 12])
                .style(win_style(hover_subtle)),
            button(text("\u{2715}").size(12))
                .on_press(Message::Exit)
                .padding([6, 12])
                .style(win_style(hover_close)),
        ]
        .spacing(0)
    };

    let mut trailing: Vec<Element<Message>> = Vec::new();
    if let Some(btn) = sftp_btn {
        trailing.push(btn);
    }
    trailing.push(add_btn);
    trailing.push(settings_btn);

    #[cfg(target_os = "windows")]
    let content = {
        let mut items: Vec<Element<Message>> = vec![tabs_scroll.into()];
        items.extend(trailing);
        items.push(window_controls.into());
        row(items).spacing(2).align_y(iced::Alignment::Center)
    };

    #[cfg(not(target_os = "windows"))]
    let content = {
        let mut items: Vec<Element<Message>> = vec![tabs_scroll.into()];
        items.extend(trailing);
        row(items).spacing(2).align_y(iced::Alignment::Center)
    };

    let bar_alpha = bar_alpha.clamp(0.0, 1.0);
    let tab_bar_container = container(content)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(Color {
                a: bar_alpha,
                ..palette.surface
            })),
            border: Border {
                radius: 0.0.into(),
                width: 0.0,
                color: Color::TRANSPARENT,
            },
            ..Default::default()
        })
        .padding(padding)
        .width(Length::Fill);

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    return mouse_area(tab_bar_container)
        .on_press(Message::WindowDrag)
        .into();

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    tab_bar_container.into()
}

fn browser_tab<'a>(
    title: &'a str,
    index: usize,
    is_active: bool,
    tab_alpha: f32,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    const MAX_TITLE_LEN: usize = 24;
    let display_title: std::borrow::Cow<'a, str> = if title.chars().count() > MAX_TITLE_LEN {
        let truncated: String = title.chars().take(MAX_TITLE_LEN - 1).collect();
        format!("{truncated}\u{2026}").into()
    } else {
        title.into()
    };
    let index_label = if index == crate::gui::app::SETTINGS_TAB_INDEX {
        text("\u{2699}".to_string()).size(10)
    } else {
        text(format!("{}", index + 1)).size(10)
    }
    .color(Color {
        a: 0.35,
        ..palette.text_secondary
    });
    let tab_text = text(display_title).size(12);

    // Background painted by `hover_fade` behind the button.
    let close_btn_inner = button(text("\u{2715}").size(9))
        .on_press(Message::CloseTab(index))
        .padding([2, 5])
        .style(
            move |_theme: &Theme, status: button::Status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                text_color: match status {
                    button::Status::Hovered => palette.text,
                    _ => Color {
                        a: 0.5,
                        ..palette.text_secondary
                    },
                },
                border: Border {
                    radius: 4.0.into(),
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
        );
    let close_rest = HoverStyle {
        background: Color::TRANSPARENT,
        border_color: Color::TRANSPARENT,
        border_width: 0.0,
        radius: 4.0,
    };
    let close_hover = HoverStyle {
        background: Color {
            a: 0.15,
            ..palette.text
        },
        ..close_rest
    };
    let close_btn = hover_fade(close_btn_inner, close_rest, close_hover, animations_enabled);

    let tab_content = row![index_label, tab_text, close_btn]
        .spacing(6)
        .align_y(iced::Alignment::Center);

    let inactive_alpha = tab_alpha.clamp(0.0, 1.0);
    // The tab background is painted by `hover_fade` so it can cross-fade on
    // hover; the button itself stays transparent.
    let tab_button_inner = button(tab_content).padding([6, 12]).style(
        move |_theme: &Theme, status: button::Status| {
            if is_active {
                button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: palette.text,
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            } else {
                let hovered = matches!(status, button::Status::Hovered);
                button::Style {
                    background: Some(Background::Color(Color::TRANSPARENT)),
                    text_color: if hovered {
                        palette.text
                    } else {
                        palette.text_secondary
                    },
                    border: Border::default(),
                    shadow: iced::Shadow::default(),
                    snap: false,
                }
            }
        },
    );

    let (rest_bg, hover_bg) = if is_active {
        let active_bg = Color {
            a: inactive_alpha,
            ..palette.background
        };
        (active_bg, active_bg)
    } else {
        (
            Color::TRANSPARENT,
            Color {
                a: 0.08,
                ..palette.text
            },
        )
    };
    let tab_button = hover_fade(
        tab_button_inner,
        HoverStyle {
            background: rest_bg,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: 0.0,
        },
        HoverStyle {
            background: hover_bg,
            border_color: Color::TRANSPARENT,
            border_width: 0.0,
            radius: 0.0,
        },
        animations_enabled,
    );

    if is_active {
        let indicator =
            container(text(""))
                .width(Length::Fill)
                .height(2)
                .style(move |_theme: &Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.6,
                        ..palette.text
                    })),
                    ..Default::default()
                });

        iced::widget::column![tab_button, indicator]
            .width(Length::Shrink)
            .into()
    } else {
        tab_button.into()
    }
}
