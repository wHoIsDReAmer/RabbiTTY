use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::accent_toggler_style;
use crate::gui::settings::{section, segmented_control};
use crate::gui::theme::{Palette, SPACING_NORMAL};
use crate::plugin::{SettingField, SettingKind};
use iced::widget::{column, row, text, text_input, toggler};
use iced::{Alignment, Element, Length};

#[derive(Debug, Default)]
pub struct PluginSettingsState {
    pub id: String,
    pub status: String,
    pub fields: Vec<(SettingField, String)>,
}

pub fn view<'a>(
    view: &'a PluginSettingsState,
    animations_enabled: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let label_width = Length::Fixed(180.0);
    let mut items: Vec<Element<Message>> = Vec::new();

    for (field, value) in &view.fields {
        items.push(field_row(
            &view.id,
            field,
            value,
            label_width,
            palette,
            animations_enabled,
        ));
    }

    let body: Element<Message> = if items.is_empty() {
        text(crate::t!("settings.plugins.no_settings"))
            .size(12)
            .color(palette.text_secondary)
            .into()
    } else {
        column(items)
            .spacing(SPACING_NORMAL)
            .width(Length::Fill)
            .into()
    };

    column![
        section(
            crate::t!("settings.plugins.status"),
            text(view.status.as_str())
                .size(13)
                .color(palette.text)
                .into(),
            palette,
        ),
        section(crate::t!("settings.plugins.settings"), body, palette),
    ]
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn field_row<'a>(
    id: &'a str,
    field: &'a SettingField,
    value: &'a str,
    label_width: Length,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let key = field.key.clone();
    let plugin = id.to_string();
    let change = move |next: String| {
        Message::Settings(SettingsMessage::PluginSettingChanged {
            plugin: plugin.clone(),
            key: key.clone(),
            value: next,
        })
    };

    let control: Element<Message> = match &field.kind {
        SettingKind::Toggle => toggler(value == "true")
            .on_toggle(move |on| change(on.to_string()))
            .size(18)
            .style(accent_toggler_style(palette))
            .into(),
        SettingKind::Select(options) => segmented_control(
            "",
            options
                .iter()
                .map(|option| (option.as_str(), change(option.clone()), option == value))
                .collect(),
            palette,
            animations_enabled,
        ),
        SettingKind::Text | SettingKind::Number(_) => text_input("", value)
            .on_input(change)
            .padding([6, 10])
            .size(13)
            .width(Length::Fill)
            .into(),
    };

    row![
        text(field.label.as_str())
            .size(13)
            .width(label_width)
            .color(palette.text),
        control,
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}
