use crate::config::SshAuthMethod;
use crate::gui::app::Message;
use crate::gui::components::{button_primary, button_secondary};
use crate::gui::settings::{
    SettingsDraft, SshConnectionTestStatus, SshProfileDraft, SshProfileField, SshProfileModalMode,
};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{
    button, center, checkbox, column, container, mouse_area, row, stack, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

pub fn view<'a>(
    draft: &'a SettingsDraft,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
) -> Element<'a, Message> {
    content(draft, ssh_config_profiles, palette)
}

pub fn modal_overlay<'a>(
    base: Element<'a, Message>,
    draft: &'a SettingsDraft,
    palette: Palette,
) -> Element<'a, Message> {
    if let Some(index) = draft.ssh_profile_delete_pending
        && let Some(profile) = draft.ssh_profiles.get(index)
    {
        return delete_confirm_overlay(base, profile, palette);
    }

    if let Some(mode) = draft.ssh_profile_modal_mode {
        return modal_overlay_content(
            base,
            mode,
            &draft.ssh_profile_modal_draft,
            draft.ssh_profiles_error.as_deref(),
            &draft.ssh_connection_test_status,
            palette,
        );
    }

    base
}

fn delete_confirm_overlay<'a>(
    base: Element<'a, Message>,
    profile: &'a SshProfileDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let backdrop = mouse_area(backdrop(palette)).on_press(Message::CancelRemoveSshProfile);
    let title = profile_title(profile);
    let description = format!("Delete \"{title}\" from SSH profiles?");

    let modal = container(
        column![
            text("Delete SSH Profile").size(16).color(palette.text),
            text(description).size(13).color(palette.text_secondary),
            row![
                container("").width(Length::Fill),
                button_secondary("Cancel", palette).on_press(Message::CancelRemoveSshProfile),
                button_primary("Delete", palette).on_press(Message::ConfirmRemoveSshProfile),
            ]
            .spacing(SPACING_SMALL)
            .align_y(Alignment::Center)
            .width(Length::Fill),
        ]
        .spacing(SPACING_NORMAL)
        .padding(20)
        .width(Length::Fixed(360.0)),
    )
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            radius: RADIUS_NORMAL.into(),
            width: 1.0,
            color: Color {
                a: 0.16,
                ..palette.text
            },
        },
        ..Default::default()
    });

    let modal_layer = mouse_area(modal).on_press(Message::Noop);

    stack![
        base,
        backdrop,
        center(modal_layer).width(Length::Fill).height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn content<'a>(
    draft: &'a SettingsDraft,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
) -> Element<'a, Message> {
    let mut items: Vec<Element<Message>> = Vec::new();

    items.push(
        row![
            text("Profiles").size(16).color(palette.text),
            container("").width(Length::Fill),
            button_primary("+", palette).on_press(Message::AddSshProfile),
        ]
        .spacing(SPACING_SMALL)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );

    if let Some(error) = &draft.ssh_profiles_error {
        items.push(status_banner(error, palette));
    }

    if draft.ssh_profiles.is_empty() {
        items.push(empty_state(palette));
    } else {
        for (index, profile) in draft.ssh_profiles.iter().enumerate() {
            items.push(profile_row(index, profile, palette));
        }
    }

    if !ssh_config_profiles.is_empty() {
        items.push(
            row![
                text("From SSH config")
                    .size(13)
                    .color(palette.text_secondary),
                container("").width(Length::Fill),
            ]
            .padding([SPACING_NORMAL, 0.0])
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
        );
        for (index, profile) in ssh_config_profiles.iter().enumerate() {
            items.push(config_profile_row(index, profile, palette));
        }
    }

    column(items)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}

fn config_profile_row<'a>(
    index: usize,
    profile: &'a crate::config::SshProfile,
    palette: Palette,
) -> Element<'a, Message> {
    let title = if !profile.name.is_empty() {
        profile.name.clone()
    } else if !profile.user.is_empty() {
        format!("{}@{}", profile.user, profile.host)
    } else {
        profile.host.clone()
    };
    let endpoint = if profile.user.is_empty() {
        format!("{}:{}", profile.host, profile.port)
    } else {
        format!("{}@{}:{}", profile.user, profile.host, profile.port)
    };
    let auth = match profile.auth_method {
        crate::config::SshAuthMethod::KeyFile => profile
            .identity_file
            .clone()
            .unwrap_or_else(|| "<key>".into()),
        crate::config::SshAuthMethod::Password => "password".into(),
    };
    let subtitle = format!("{endpoint}  •  {auth}");

    container(
        row![
            column![
                text(title).size(14).color(palette.text),
                text(subtitle).size(12).color(palette.text_secondary),
            ]
            .spacing(4)
            .width(Length::Fill),
            row![icon_button("\u{25b6}", palette).on_press(Message::CreateSshTabFromConfig(index)),]
                .spacing(6)
                .align_y(Alignment::Center),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.06,
            ..palette.surface
        })),
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 1.0,
            color: Color {
                a: 0.06,
                ..palette.text
            },
        },
        ..Default::default()
    })
    .into()
}

fn empty_state(palette: Palette) -> Element<'static, Message> {
    container(
        column![
            text("\u{21c4}").size(28).color(Color {
                a: 0.3,
                ..palette.text
            }),
            text("No SSH profiles yet").size(13).color(Color {
                a: 0.5,
                ..palette.text
            }),
            text("Add a profile to quickly connect to remote servers")
                .size(11)
                .color(palette.text_secondary),
        ]
        .spacing(6)
        .align_x(Alignment::Center),
    )
    .center_x(Length::Fill)
    .padding([24, 0])
    .into()
}

fn profile_row<'a>(
    index: usize,
    profile: &'a SshProfileDraft,
    palette: Palette,
) -> Element<'a, Message> {
    let title = profile_title(profile);
    let subtitle = profile_subtitle(profile);

    container(
        row![
            column![
                text(title).size(14).color(palette.text),
                text(subtitle).size(12).color(palette.text_secondary),
            ]
            .spacing(4)
            .width(Length::Fill),
            row![
                icon_button("\u{25b6}", palette).on_press(Message::CreateSshTab(index)),
                icon_button("\u{270e}", palette).on_press(Message::EditSshProfile(index)),
                icon_button("\u{1f5d1}", palette).on_press(Message::RequestRemoveSshProfile(index)),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        ]
        .spacing(SPACING_NORMAL)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(Color {
            a: 0.10,
            ..palette.surface
        })),
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 1.0,
            color: Color {
                a: 0.08,
                ..palette.text
            },
        },
        ..Default::default()
    })
    .into()
}

fn profile_title(profile: &SshProfileDraft) -> String {
    if !profile.name.trim().is_empty() {
        profile.name.trim().to_string()
    } else if !profile.host.trim().is_empty() && !profile.user.trim().is_empty() {
        format!("{}@{}", profile.user.trim(), profile.host.trim())
    } else if !profile.host.trim().is_empty() {
        profile.host.trim().to_string()
    } else {
        "New Profile".to_string()
    }
}

fn profile_subtitle(profile: &SshProfileDraft) -> String {
    let endpoint = if profile.user.trim().is_empty() {
        format!(
            "{}:{}",
            empty_label(&profile.host),
            empty_label(&profile.port)
        )
    } else {
        format!(
            "{}@{}:{}",
            profile.user.trim(),
            empty_label(&profile.host),
            empty_label(&profile.port)
        )
    };
    let auth = match profile.auth_method {
        SshAuthMethod::KeyFile => "Key file",
        SshAuthMethod::Password => "Password",
    };
    let proxy = if profile.proxy_command.trim().is_empty() {
        ""
    } else {
        " · Proxy"
    };
    format!("{endpoint} · {auth}{proxy}")
}

fn empty_label(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() { "-" } else { value }
}

fn modal_overlay_content<'a>(
    base: Element<'a, Message>,
    mode: SshProfileModalMode,
    profile: &'a SshProfileDraft,
    error: Option<&'a str>,
    test_status: &'a SshConnectionTestStatus,
    palette: Palette,
) -> Element<'a, Message> {
    let backdrop = mouse_area(backdrop(palette)).on_press(Message::CloseSshProfileModal);

    let title = match mode {
        SshProfileModalMode::Create => "Create SSH Profile",
        SshProfileModalMode::Edit(_) => "Edit SSH Profile",
    };

    let mut modal_items: Vec<Element<Message>> = Vec::new();
    modal_items.push(
        row![
            text(title).size(16).color(palette.text),
            container("").width(Length::Fill),
            icon_button("x", palette).on_press(Message::CloseSshProfileModal),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );
    if let Some(error) = error.filter(|message| *message != "SSH profiles saved.") {
        modal_items.push(status_banner(error, palette));
    }
    modal_items.push(profile_form(profile, palette));
    if let Some(status) = connection_test_status_banner(test_status, palette) {
        modal_items.push(status);
    }
    let test_button = if matches!(test_status, SshConnectionTestStatus::Testing) {
        button_secondary("Testing...", palette)
    } else {
        button_secondary("Test Connection", palette).on_press(Message::TestSshConnection)
    };
    modal_items.push(container("").height(Length::Fixed(8.0)).into());
    modal_items.push(
        row![
            test_button,
            container("").width(Length::Fill),
            button_secondary("Cancel", palette).on_press(Message::CloseSshProfileModal),
            button_primary("Save", palette).on_press(Message::SaveSshProfileModal),
        ]
        .spacing(SPACING_SMALL)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );

    let modal = container(
        column(modal_items)
            .spacing(SPACING_NORMAL)
            .padding(20)
            .width(Length::Fixed(480.0)),
    )
    .style(move |_theme: &iced::Theme| container::Style {
        background: Some(Background::Color(palette.surface)),
        border: Border {
            radius: RADIUS_NORMAL.into(),
            width: 1.0,
            color: Color {
                a: 0.16,
                ..palette.text
            },
        },
        ..Default::default()
    });

    let modal_layer = mouse_area(modal).on_press(Message::Noop);

    stack![
        base,
        backdrop,
        center(modal_layer).width(Length::Fill).height(Length::Fill)
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn connection_test_status_banner<'a>(
    status: &'a SshConnectionTestStatus,
    palette: Palette,
) -> Option<Element<'a, Message>> {
    match status {
        SshConnectionTestStatus::Idle => None,
        SshConnectionTestStatus::Testing => Some(status_banner("Testing connection...", palette)),
        SshConnectionTestStatus::Success(message) => Some(status_banner(message, palette)),
        SshConnectionTestStatus::Failure(message) => Some(status_banner(message, palette)),
    }
}

fn backdrop(palette: Palette) -> container::Container<'static, Message> {
    container(text(""))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                r: palette.background.r,
                g: palette.background.g,
                b: palette.background.b,
                a: 0.50,
            })),
            ..Default::default()
        })
}

fn profile_form<'a>(profile: &'a SshProfileDraft, palette: Palette) -> Element<'a, Message> {
    let key_button = auth_method_button(
        "Key File",
        matches!(profile.auth_method, SshAuthMethod::KeyFile),
        "key_file",
        palette,
    );

    let password_button = auth_method_button(
        "Password",
        matches!(profile.auth_method, SshAuthMethod::Password),
        "password",
        palette,
    );

    let auth_input: Element<'a, Message> = if matches!(profile.auth_method, SshAuthMethod::KeyFile)
    {
        modal_input(
            "Key File  (e.g. ~/.ssh/id_rsa)",
            &profile.identity_file,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::IdentityFile, next),
            palette,
        )
        .into()
    } else {
        modal_password("Password", &profile.password, palette).into()
    };

    let mut items: Vec<Element<Message>> = Vec::new();
    items.push(
        modal_input(
            "Display Name (optional)",
            &profile.name,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::Name, next),
            palette,
        )
        .into(),
    );
    items.push(
        row![
            modal_input(
                "Host",
                &profile.host,
                |next| { Message::SshProfileModalFieldChanged(SshProfileField::Host, next) },
                palette
            )
            .width(Length::Fill),
            text(":").size(13).color(palette.text_secondary),
            modal_input(
                "Port",
                &profile.port,
                |next| { Message::SshProfileModalFieldChanged(SshProfileField::Port, next) },
                palette
            )
            .width(Length::Fixed(80.0)),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );
    items.push(
        modal_input(
            "Username",
            &profile.user,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::User, next),
            palette,
        )
        .into(),
    );
    items.push(
        text("Authentication")
            .size(11)
            .color(palette.text_secondary)
            .into(),
    );
    items.push(
        row![key_button, password_button]
            .spacing(SPACING_SMALL)
            .width(Length::Fill)
            .into(),
    );
    items.push(auth_input);
    items.push(
        text(if matches!(profile.auth_method, SshAuthMethod::Password) {
            "Password is stored securely in your OS keychain"
        } else {
            "Key file path is stored in config"
        })
        .size(10)
        .color(Color {
            a: 0.35,
            ..palette.text
        })
        .into(),
    );
    items.push(
        checkbox(profile.proxy_command_enabled)
            .label("Use Proxy Command")
            .on_toggle(|enabled| {
                Message::SshProfileModalFieldChanged(
                    SshProfileField::ProxyCommandEnabled,
                    enabled.to_string(),
                )
            })
            .size(14)
            .text_size(13)
            .into(),
    );
    if profile.proxy_command_enabled {
        items.push(
            modal_input(
                "ProxyCommand  (e.g. cloudflared access ssh --hostname %h)",
                &profile.proxy_command,
                |next| Message::SshProfileModalFieldChanged(SshProfileField::ProxyCommand, next),
                palette,
            )
            .into(),
        );
        items.push(
            text("%h and %p are replaced with host and port")
                .size(10)
                .color(Color {
                    a: 0.35,
                    ..palette.text
                })
                .into(),
        );
    }

    column(items).spacing(8).width(Length::Fill).into()
}

fn auth_method_button<'a>(
    label: &'a str,
    selected: bool,
    value: &'static str,
    palette: Palette,
) -> button::Button<'a, Message> {
    let message = Message::SshProfileModalFieldChanged(SshProfileField::AuthMethod, value.into());
    if selected {
        button_primary(label, palette).on_press(message)
    } else {
        button_secondary(label, palette).on_press(message)
    }
}

fn status_banner<'a>(message: &'a str, palette: Palette) -> Element<'a, Message> {
    let is_saved = message == "SSH profiles saved."
        || message == "Connection successful."
        || message == "Testing connection...";
    let color = if is_saved {
        palette.accent
    } else {
        palette.error
    };

    container(text(message).size(12).color(Color { a: 0.95, ..color }))
        .padding([8, 10])
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.08, ..color })),
            border: Border {
                radius: RADIUS_SMALL.into(),
                width: 1.0,
                color: Color { a: 0.25, ..color },
            },
            ..Default::default()
        })
        .into()
}

fn icon_button(icon: &str, palette: Palette) -> button::Button<'_, Message> {
    button(text(icon).size(15)).padding([5, 8]).style(
        move |_theme: &iced::Theme, status: button::Status| {
            let (bg, border_alpha) = match status {
                button::Status::Hovered => (
                    Color {
                        a: 0.12,
                        ..palette.text
                    },
                    0.20,
                ),
                button::Status::Pressed => (
                    Color {
                        a: 0.08,
                        ..palette.text
                    },
                    0.25,
                ),
                button::Status::Disabled => (
                    Color {
                        a: 0.04,
                        ..palette.text
                    },
                    0.05,
                ),
                _ => (Color::TRANSPARENT, 0.10),
            };
            let text_color = match status {
                button::Status::Disabled => palette.text_secondary,
                _ => palette.text,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color,
                border: Border {
                    radius: RADIUS_SMALL.into(),
                    width: 1.0,
                    color: Color {
                        a: border_alpha,
                        ..palette.text
                    },
                },
                shadow: iced::Shadow::default(),
                snap: true,
            }
        },
    )
}

fn modal_input<'a, F>(
    placeholder: &'a str,
    value: &'a str,
    on_input: F,
    palette: Palette,
) -> text_input::TextInput<'a, Message>
where
    F: 'a + Fn(String) -> Message,
{
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([6, 10])
        .size(13)
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme, status: text_input::Status| input_style(palette, status))
}

fn modal_password<'a>(
    placeholder: &'a str,
    value: &'a str,
    palette: Palette,
) -> text_input::TextInput<'a, Message> {
    text_input(placeholder, value)
        .secure(true)
        .on_input(move |next| Message::SshProfileModalFieldChanged(SshProfileField::Password, next))
        .padding([6, 10])
        .size(13)
        .width(Length::Fill)
        .style(move |_theme: &iced::Theme, status: text_input::Status| input_style(palette, status))
}

fn input_style(palette: Palette, status: text_input::Status) -> text_input::Style {
    let focused = matches!(status, text_input::Status::Focused { .. });
    text_input::Style {
        background: Background::Color(Color {
            a: 0.25,
            ..palette.background
        }),
        border: Border {
            radius: RADIUS_SMALL.into(),
            width: 1.0,
            color: if focused {
                Color {
                    a: 0.5,
                    ..palette.accent
                }
            } else {
                Color {
                    a: 0.08,
                    ..palette.text
                }
            },
        },
        icon: palette.text_secondary,
        placeholder: Color {
            a: 0.3,
            ..palette.text
        },
        value: palette.text,
        selection: Color {
            a: 0.3,
            ..palette.accent
        },
    }
}
