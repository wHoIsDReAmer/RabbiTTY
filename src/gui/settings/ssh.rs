use crate::config::SshAuthMethod;
use crate::gui::app::Message;
use crate::gui::components::{button_icon, primary, secondary};
use crate::gui::settings::{
    SettingsDraft, SshConnectionTestStatus, SshProfileDraft, SshProfileField, SshProfileModalMode,
};
use crate::gui::theme::{Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{center, checkbox, column, container, mouse_area, row, stack, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length};

pub fn view<'a>(
    draft: &'a SettingsDraft,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    content(draft, ssh_config_profiles, palette, animations_enabled)
}

pub fn modal_overlay<'a>(
    base: Element<'a, Message>,
    draft: &'a SettingsDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    if let Some(index) = draft.ssh_profile_delete_pending
        && let Some(profile) = draft.ssh_profiles.get(index)
    {
        return delete_confirm_overlay(base, profile, palette, animations_enabled);
    }

    if let Some(mode) = draft.ssh_profile_modal_mode {
        return modal_overlay_content(
            base,
            mode,
            &draft.ssh_profile_modal_draft,
            draft.ssh_profiles_error.as_deref(),
            &draft.ssh_connection_test_status,
            palette,
            animations_enabled,
        );
    }

    base
}

fn delete_confirm_overlay<'a>(
    base: Element<'a, Message>,
    profile: &'a SshProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let backdrop = mouse_area(backdrop(palette)).on_press(Message::CancelRemoveSshProfile);
    let title = profile_title(profile);
    let description = format!("Delete \"{title}\" from SSH profiles?");

    let modal = container(
        column![
            text(crate::t!("settings.ssh.delete_profile"))
                .size(16)
                .color(palette.text),
            text(description).size(13).color(palette.text_secondary),
            row![
                container("").width(Length::Fill),
                secondary(
                    crate::t!("settings.ssh.cancel"),
                    Some(Message::CancelRemoveSshProfile),
                    palette,
                    animations_enabled,
                ),
                primary(
                    crate::t!("settings.ssh.delete"),
                    Message::ConfirmRemoveSshProfile,
                    palette,
                    animations_enabled,
                ),
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
    animations_enabled: bool,
) -> Element<'a, Message> {
    let mut items: Vec<Element<Message>> = Vec::new();

    items.push(
        row![
            text(crate::t!("settings.ssh.profiles"))
                .size(16)
                .color(palette.text),
            container("").width(Length::Fill),
            primary("+", Message::AddSshProfile, palette, animations_enabled),
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
            items.push(profile_row(index, profile, palette, animations_enabled));
        }
    }

    if !ssh_config_profiles.is_empty() {
        items.push(
            row![
                text(crate::t!("settings.ssh.from_ssh_config"))
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
            items.push(config_profile_row(
                index,
                profile,
                palette,
                animations_enabled,
            ));
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
    animations_enabled: bool,
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
            row![button_icon(
                "\u{25b6}",
                Message::CreateSshTabFromConfig(index),
                palette,
                animations_enabled,
            ),]
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
            text(crate::t!("settings.ssh.no_profiles"))
                .size(13)
                .color(Color {
                    a: 0.5,
                    ..palette.text
                }),
            text(crate::t!("settings.ssh.empty_hint"))
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
    animations_enabled: bool,
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
                button_icon(
                    "\u{25b6}",
                    Message::CreateSshTab(index),
                    palette,
                    animations_enabled,
                ),
                button_icon(
                    "\u{270e}",
                    Message::EditSshProfile(index),
                    palette,
                    animations_enabled,
                ),
                button_icon(
                    "\u{1f5d1}",
                    Message::RequestRemoveSshProfile(index),
                    palette,
                    animations_enabled,
                ),
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
        crate::t!("settings.ssh.new_profile").to_string()
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
        SshAuthMethod::KeyFile => crate::t!("settings.ssh.key_file"),
        SshAuthMethod::Password => crate::t!("settings.ssh.password"),
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

#[allow(clippy::too_many_arguments)]
fn modal_overlay_content<'a>(
    base: Element<'a, Message>,
    mode: SshProfileModalMode,
    profile: &'a SshProfileDraft,
    error: Option<&'a str>,
    test_status: &'a SshConnectionTestStatus,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let backdrop = mouse_area(backdrop(palette)).on_press(Message::CloseSshProfileModal);

    let title = match mode {
        SshProfileModalMode::Create => crate::t!("settings.ssh.create_profile"),
        SshProfileModalMode::Edit(_) => crate::t!("settings.ssh.edit_profile"),
    };

    let mut modal_items: Vec<Element<Message>> = Vec::new();
    modal_items.push(
        row![
            text(title).size(16).color(palette.text),
            container("").width(Length::Fill),
            button_icon(
                "x",
                Message::CloseSshProfileModal,
                palette,
                animations_enabled
            ),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );
    if let Some(error) =
        error.filter(|message| *message != crate::t!("settings.ssh.status.profiles_saved"))
    {
        modal_items.push(status_banner(error, palette));
    }
    modal_items.push(profile_form(profile, palette, animations_enabled));
    if let Some(status) = connection_test_status_banner(test_status, palette) {
        modal_items.push(status);
    }
    let test_button: Element<Message> = if matches!(test_status, SshConnectionTestStatus::Testing) {
        secondary(
            crate::t!("settings.ssh.testing"),
            None,
            palette,
            animations_enabled,
        )
    } else {
        secondary(
            crate::t!("settings.ssh.test_connection"),
            Some(Message::TestSshConnection),
            palette,
            animations_enabled,
        )
    };
    modal_items.push(container("").height(Length::Fixed(8.0)).into());
    modal_items.push(
        row![
            test_button,
            container("").width(Length::Fill),
            secondary(
                crate::t!("settings.ssh.cancel"),
                Some(Message::CloseSshProfileModal),
                palette,
                animations_enabled,
            ),
            primary(
                crate::t!("settings.ssh.save"),
                Message::SaveSshProfileModal,
                palette,
                animations_enabled,
            ),
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
        SshConnectionTestStatus::Testing => Some(status_banner(
            crate::t!("settings.ssh.status.testing_connection"),
            palette,
        )),
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

fn profile_form<'a>(
    profile: &'a SshProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let key_button = auth_method_button(
        crate::t!("settings.ssh.key_file_label"),
        matches!(profile.auth_method, SshAuthMethod::KeyFile),
        "key_file",
        palette,
        animations_enabled,
    );

    let password_button = auth_method_button(
        crate::t!("settings.ssh.password"),
        matches!(profile.auth_method, SshAuthMethod::Password),
        "password",
        palette,
        animations_enabled,
    );

    let auth_input: Element<'a, Message> = if matches!(profile.auth_method, SshAuthMethod::KeyFile)
    {
        modal_input(
            crate::t!("settings.ssh.key_file_input_placeholder"),
            &profile.identity_file,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::IdentityFile, next),
            palette,
        )
        .into()
    } else {
        modal_password(
            crate::t!("settings.ssh.password"),
            &profile.password,
            palette,
        )
        .into()
    };

    let mut items: Vec<Element<Message>> = Vec::new();
    items.push(
        modal_input(
            crate::t!("settings.ssh.display_name"),
            &profile.name,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::Name, next),
            palette,
        )
        .into(),
    );
    items.push(
        row![
            modal_input(
                crate::t!("settings.ssh.host"),
                &profile.host,
                |next| { Message::SshProfileModalFieldChanged(SshProfileField::Host, next) },
                palette
            )
            .width(Length::Fill),
            text(":").size(13).color(palette.text_secondary),
            modal_input(
                crate::t!("settings.ssh.port"),
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
            crate::t!("settings.ssh.username"),
            &profile.user,
            |next| Message::SshProfileModalFieldChanged(SshProfileField::User, next),
            palette,
        )
        .into(),
    );
    items.push(
        text(crate::t!("settings.ssh.authentication"))
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
            crate::t!("settings.ssh.auth_hint_password")
        } else {
            crate::t!("settings.ssh.auth_hint_key_file")
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
            .label(crate::t!("settings.ssh.use_proxy_command"))
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
                crate::t!("settings.ssh.proxy_command_placeholder"),
                &profile.proxy_command,
                |next| Message::SshProfileModalFieldChanged(SshProfileField::ProxyCommand, next),
                palette,
            )
            .into(),
        );
        items.push(
            text(crate::t!("settings.ssh.proxy_command_hint"))
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
    animations_enabled: bool,
) -> Element<'a, Message> {
    let message = Message::SshProfileModalFieldChanged(SshProfileField::AuthMethod, value.into());
    if selected {
        primary(label, message, palette, animations_enabled)
    } else {
        secondary(label, Some(message), palette, animations_enabled)
    }
}

fn status_banner<'a>(message: &'a str, palette: Palette) -> Element<'a, Message> {
    let is_saved = message == crate::t!("settings.ssh.status.profiles_saved")
        || message == crate::t!("settings.ssh.status.connection_successful")
        || message == crate::t!("settings.ssh.status.testing_connection");
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
