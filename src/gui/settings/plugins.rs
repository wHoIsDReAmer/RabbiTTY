use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::{accent_toggler_style, secondary};
use crate::gui::settings::{
    NUMERIC_INPUT_WIDTH, ROW_SPACING, SECTION_SPACING, TEXT_INPUT_WIDTH, section,
    segmented_control, setting_row, styled_text_input,
};
use crate::gui::theme::{Palette, SPACING_NORMAL, SPACING_SMALL};
use crate::plugin::{SettingField, SettingKind};
use iced::widget::{Space, button, column, container, row, text, toggler};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Ready,
    #[default]
    Disabled,
    Retired,
}

#[derive(Debug, Clone)]
pub struct PluginPermission {
    pub name: String,
    pub granted: bool,
    pub optional: bool,
}

#[derive(Debug, Default)]
pub struct PluginSettingsState {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub enabled: bool,
    pub state: PluginState,
    pub failure: Option<String>,
    pub permissions: Vec<PluginPermission>,
    pub fields: Vec<(SettingField, String)>,
}

pub fn view<'a>(
    view: &'a PluginSettingsState,
    animations_enabled: bool,
    palette: Palette,
) -> Element<'a, Message> {
    let mut items: Vec<Element<Message>> = vec![header(view, palette, animations_enabled)];

    if let Some(reason) = &view.failure {
        items.push(failure_notice(
            &view.id,
            reason,
            palette,
            animations_enabled,
        ));
    }

    if !view.permissions.is_empty() {
        items.push(section(
            crate::t!("settings.plugins.permissions"),
            permission_list(view, palette, animations_enabled),
            palette,
        ));
    }

    items.push(section(
        crate::t!("settings.plugins.settings"),
        field_list(view, palette, animations_enabled),
        palette,
    ));

    column(items)
        .spacing(SECTION_SPACING)
        .width(Length::Fill)
        .into()
}

fn header<'a>(
    view: &'a PluginSettingsState,
    palette: Palette,
    _animations_enabled: bool,
) -> Element<'a, Message> {
    let plugin = view.id.clone();
    let identity = column![
        row![
            text(view.name.as_str()).size(22).color(palette.text),
            badge(view.state, palette),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_SMALL),
        text(version_line(view))
            .size(12)
            .color(palette.text_secondary),
    ]
    .spacing(4)
    .width(Length::Fill);

    let identity = match &view.description {
        Some(description) => identity.push(
            text(description.as_str())
                .size(12)
                .color(palette.text_secondary),
        ),
        None => identity,
    };

    let identity = match &view.homepage {
        Some(url) => identity.push(link(url, palette)),
        None => identity,
    };

    row![
        identity,
        toggler(view.enabled)
            .on_toggle(
                move |enabled| Message::Settings(SettingsMessage::PluginToggled {
                    plugin: plugin.clone(),
                    enabled,
                })
            )
            .size(20)
            .style(accent_toggler_style(palette)),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}

fn version_line(view: &PluginSettingsState) -> String {
    let mut parts = vec![view.id.clone()];
    if !view.version.is_empty() {
        parts.push(format!("v{}", view.version));
    }
    if let Some(author) = &view.author {
        parts.push(author.clone());
    }
    parts.join("  ·  ")
}

fn link<'a>(url: &'a str, palette: Palette) -> Element<'a, Message> {
    button(text(url).size(12).color(palette.accent))
        .style(|_theme: &Theme, _status| iced::widget::button::Style {
            background: Some(Background::Color(Color::TRANSPARENT)),
            text_color: Color::WHITE,
            border: Border::default(),
            shadow: iced::Shadow::default(),
            snap: false,
        })
        .padding(0)
        .on_press(Message::OpenUrl(url.to_string()))
        .into()
}

fn badge<'a>(state: PluginState, palette: Palette) -> Element<'a, Message> {
    let (label, color) = match state {
        PluginState::Ready => (crate::t!("settings.plugins.ready"), palette.success),
        PluginState::Disabled => (
            crate::t!("settings.plugins.disabled"),
            palette.text_secondary,
        ),
        PluginState::Retired => (crate::t!("settings.plugins.retired"), palette.error),
    };

    container(text(label).size(11).color(color))
        .padding([2, 8])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.14, ..color })),
            border: Border {
                radius: 10.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn failure_notice<'a>(
    id: &'a str,
    reason: &'a str,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    container(
        row![
            column![
                text(crate::t!("settings.plugins.stopped"))
                    .size(13)
                    .color(palette.error),
                text(reason).size(12).color(palette.text_secondary),
            ]
            .spacing(4)
            .width(Length::Fill),
            secondary(
                crate::t!("settings.plugins.reload"),
                Some(Message::Settings(SettingsMessage::PluginReloaded(
                    id.to_string(),
                ))),
                palette,
                animations_enabled,
            ),
        ]
        .align_y(Alignment::Center)
        .spacing(SPACING_NORMAL),
    )
    .padding(SPACING_NORMAL)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.10,
            ..palette.error
        })),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .into()
}

fn permission_list<'a>(
    view: &'a PluginSettingsState,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let rows: Vec<Element<Message>> = view
        .permissions
        .iter()
        .map(|permission| permission_row(&view.id, permission, palette, animations_enabled))
        .collect();

    column(rows).spacing(ROW_SPACING).width(Length::Fill).into()
}

fn permission_row<'a>(
    id: &'a str,
    permission: &'a PluginPermission,
    palette: Palette,
    _animations_enabled: bool,
) -> Element<'a, Message> {
    let detail = if permission.optional {
        crate::t!("settings.plugins.needs_consent")
    } else {
        crate::t!("settings.plugins.auto_granted")
    };

    let label = column![
        text(capability_label(&permission.name))
            .size(14)
            .color(palette.text),
        text(detail).size(12).color(palette.text_secondary),
    ]
    .spacing(SPACING_SMALL);

    let control: Element<Message> = if permission.optional {
        let plugin = id.to_string();
        let capability = permission.name.clone();
        toggler(permission.granted)
            .on_toggle(move |granted| {
                Message::Settings(SettingsMessage::PluginConsentChanged {
                    plugin: plugin.clone(),
                    capability: capability.clone(),
                    granted,
                })
            })
            .size(18)
            .style(accent_toggler_style(palette))
            .into()
    } else {
        crate::gui::icons::ui(crate::gui::icons::Ui::Check, 14.0, palette.success)
    };

    row![label, Space::new().width(Length::Fill), control]
        .align_y(Alignment::Center)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}

fn capability_label(name: &str) -> &'static str {
    match name {
        "write-pty" => crate::t!("settings.plugins.capability.write_pty"),
        "read-config" => crate::t!("settings.plugins.capability.read_config"),
        "notify" => crate::t!("settings.plugins.capability.notify"),
        "network" => crate::t!("settings.plugins.capability.network"),
        "filesystem" => crate::t!("settings.plugins.capability.filesystem"),
        _ => "",
    }
}

fn field_list<'a>(
    view: &'a PluginSettingsState,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    if view.fields.is_empty() {
        return text(crate::t!("settings.plugins.no_settings"))
            .size(12)
            .color(palette.text_secondary)
            .into();
    }

    let rows: Vec<Element<Message>> = view
        .fields
        .iter()
        .map(|(field, value)| field_row(&view.id, field, value, palette, animations_enabled))
        .collect();

    column(rows).spacing(ROW_SPACING).width(Length::Fill).into()
}

fn field_row<'a>(
    id: &'a str,
    field: &'a SettingField,
    value: &'a str,
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
    let label = field.label.as_str();

    match &field.kind {
        SettingKind::Toggle => setting_row(
            label,
            toggler(value == "true")
                .on_toggle(move |on| change(on.to_string()))
                .size(18)
                .style(accent_toggler_style(palette)),
            palette,
        ),
        SettingKind::Select(options) => segmented_control(
            label,
            options
                .iter()
                .map(|option| (option.as_str(), change(option.clone()), option == value))
                .collect(),
            palette,
            animations_enabled,
        ),
        SettingKind::Number(_) => setting_row(
            label,
            styled_text_input(value, change, palette).width(Length::Fixed(NUMERIC_INPUT_WIDTH)),
            palette,
        ),
        SettingKind::Text => setting_row(
            label,
            styled_text_input(value, change, palette).width(Length::Fixed(TEXT_INPUT_WIDTH)),
            palette,
        ),
    }
}
