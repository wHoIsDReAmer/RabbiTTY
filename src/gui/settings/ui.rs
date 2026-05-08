use crate::config::AppConfig;
use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SettingsField, input_row_with_suffix, section};
use crate::gui::theme::{Palette, RADIUS_SMALL, SPACING_NORMAL};
use crate::i18n::AVAILABLE_LOCALES;
use iced::widget::{button, column, row, text};
use iced::{Background, Border, Color, Element, Length};

pub fn view<'a>(
    _config: &'a AppConfig,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let language_section = section(
        t!("settings.language.section_title"),
        language_picker(&draft.language, palette),
        palette,
    );

    let window_section = section(
        t!("settings.ui.window_section"),
        column(vec![
            input_row_with_suffix(
                t!("settings.ui.width"),
                &draft.window_width,
                SettingsField::UiWindowWidth,
                "px",
                palette,
            ),
            input_row_with_suffix(
                t!("settings.ui.height"),
                &draft.window_height,
                SettingsField::UiWindowHeight,
                "px",
                palette,
            ),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into(),
        palette,
    );

    column(vec![language_section, window_section])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}

fn language_picker<'a>(current: &'a str, palette: Palette) -> Element<'a, Message> {
    let mut buttons: Vec<Element<'a, Message>> = Vec::with_capacity(AVAILABLE_LOCALES.len() + 1);
    buttons.push(language_button(
        "auto",
        t!("settings.language.auto"),
        current,
        palette,
    ));
    for locale in AVAILABLE_LOCALES {
        buttons.push(language_button(
            locale.tag,
            locale.native_label,
            current,
            palette,
        ));
    }
    row(buttons).spacing(8).width(Length::Fill).into()
}

fn language_button<'a>(
    tag: &'static str,
    label: &'static str,
    current: &str,
    palette: Palette,
) -> Element<'a, Message> {
    let is_selected = current == tag;
    let accent = palette.accent;
    let text_color = palette.text;
    let surface = palette.surface;

    button(
        text(label)
            .size(13)
            .color(if is_selected { accent } else { text_color }),
    )
    .on_press(Message::SettingsInputChanged(
        SettingsField::UiLanguage,
        tag.to_string(),
    ))
    .padding([6, 14])
    .style(move |_theme: &iced::Theme, _status| button::Style {
        background: Some(Background::Color(Color {
            a: if is_selected { 0.25 } else { 0.12 },
            ..surface
        })),
        text_color: if is_selected { accent } else { text_color },
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: if is_selected { 1.5 } else { 1.0 },
            color: if is_selected {
                accent
            } else {
                Color {
                    a: 0.15,
                    ..text_color
                }
            },
        },
        shadow: Default::default(),
        snap: true,
    })
    .into()
}
