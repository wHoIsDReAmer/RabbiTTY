use crate::config::SshAuthMethod;
use crate::gui::app::{Message, SettingsMessage};
use crate::gui::components::{
    accent_combo_box_menu_style, accent_pick_list_style, button_icon, icon_toggle_content, primary,
    secondary,
};
use crate::gui::icons;
use crate::gui::settings::{
    ProfileDraft, ProfileDraftKind, ProfileField, ProfileModalMode, ProfileModalTab, SettingsDraft,
    SshConnectionTestStatus,
};
use crate::gui::theme::{
    Palette, RADIUS_NORMAL, RADIUS_SMALL, SPACING_LARGE, SPACING_NORMAL, SPACING_SMALL,
};
use iced::widget::{
    center, checkbox, column, container, mouse_area, pick_list, row, stack, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

pub fn view<'a>(
    draft: &'a SettingsDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    content(draft, palette, animations_enabled)
}

pub fn modal_overlay<'a>(
    base: Element<'a, Message>,
    draft: &'a SettingsDraft,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    progress: f32,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    if let Some(index) = draft.profile_delete_pending
        && let Some(profile) = draft.profiles.get(index)
    {
        return delete_confirm_overlay(base, profile, palette, animations_enabled);
    }

    if let Some(mode) = draft.profile_modal_mode {
        return modal_overlay_content(
            base,
            mode,
            draft,
            ssh_config_profiles,
            progress,
            palette,
            animations_enabled,
        );
    }

    base
}

fn delete_confirm_overlay<'a>(
    base: Element<'a, Message>,
    profile: &'a ProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let backdrop = mouse_area(backdrop(palette, 1.0))
        .on_press(Message::Settings(SettingsMessage::CancelRemoveProfile));
    let title = profile_title(profile);
    let description = format!("Delete \"{title}\"?");

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
                    Some(Message::Settings(SettingsMessage::CancelRemoveProfile)),
                    palette,
                    animations_enabled,
                ),
                primary(
                    crate::t!("settings.ssh.delete"),
                    Message::Settings(SettingsMessage::ConfirmRemoveProfile),
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
            primary(
                "+",
                Message::Settings(SettingsMessage::AddProfile),
                palette,
                animations_enabled
            ),
        ]
        .spacing(SPACING_SMALL)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );

    if let Some(error) = &draft.profiles_error {
        items.push(status_banner(error, palette));
    }

    if draft.profiles.is_empty() {
        items.push(empty_state(palette));
    } else {
        for (index, profile) in draft.profiles.iter().enumerate() {
            items.push(profile_row(index, profile, palette, animations_enabled));
        }
    }

    column(items)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
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

fn profile_icon<'a>(profile: &ProfileDraft) -> Element<'a, Message> {
    let icon = match profile.icon.trim() {
        "" => match profile.kind {
            ProfileDraftKind::Local => icons::by_name(&icons::default_shell_name()),
            ProfileDraftKind::Ssh => icons::ssh(),
        },
        name => icons::by_name(name),
    };
    container(icons::view(icon, 16.0, 1.0))
        .width(Length::Fixed(22.0))
        .align_x(Alignment::Center)
        .into()
}

fn profile_row<'a>(
    index: usize,
    profile: &'a ProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let title = profile_title(profile);
    let subtitle = profile_subtitle(profile);

    container(
        row![
            profile_icon(profile),
            column![
                text(title).size(14).color(palette.text),
                text(subtitle).size(12).color(palette.text_secondary),
            ]
            .spacing(4)
            .width(Length::Fill),
            row![
                button_icon(
                    "\u{25b6}",
                    Message::Settings(SettingsMessage::LaunchProfile(index)),
                    palette,
                    animations_enabled,
                ),
                button_icon(
                    "\u{270e}",
                    Message::Settings(SettingsMessage::EditProfile(index)),
                    palette,
                    animations_enabled,
                ),
                button_icon(
                    "\u{1f5d1}",
                    Message::Settings(SettingsMessage::RequestRemoveProfile(index)),
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

fn profile_title(profile: &ProfileDraft) -> String {
    if !profile.name.trim().is_empty() {
        return profile.name.trim().to_string();
    }
    match profile.kind {
        ProfileDraftKind::Local => {
            let program = profile.program.trim();
            if program.is_empty() {
                crate::t!("settings.ssh.default_shell").to_string()
            } else {
                program.to_string()
            }
        }
        ProfileDraftKind::Ssh => {
            if !profile.host.trim().is_empty() && !profile.user.trim().is_empty() {
                format!("{}@{}", profile.user.trim(), profile.host.trim())
            } else if !profile.host.trim().is_empty() {
                profile.host.trim().to_string()
            } else {
                crate::t!("settings.ssh.new_profile").to_string()
            }
        }
    }
}

fn profile_subtitle(profile: &ProfileDraft) -> String {
    match profile.kind {
        ProfileDraftKind::Local => {
            let program = profile.program.trim();
            if program.is_empty() {
                crate::t!("settings.ssh.default_shell").to_string()
            } else {
                program.to_string()
            }
        }
        ProfileDraftKind::Ssh => {
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
    }
}

fn empty_label(value: &str) -> &str {
    let value = value.trim();
    if value.is_empty() { "-" } else { value }
}

#[allow(clippy::too_many_arguments)]
fn modal_overlay_content<'a>(
    base: Element<'a, Message>,
    mode: ProfileModalMode,
    draft: &'a SettingsDraft,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    progress: f32,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let profile = &draft.profile_modal_draft;
    let error = draft.profiles_error.as_deref();
    let test_status = &draft.ssh_connection_test_status;
    let backdrop = mouse_area(backdrop(palette, progress))
        .on_press(Message::Settings(SettingsMessage::CloseProfileModal));

    let title = match mode {
        ProfileModalMode::Create => crate::t!("settings.ssh.create_profile"),
        ProfileModalMode::Edit(_) => crate::t!("settings.ssh.edit_profile"),
    };

    let is_ssh = matches!(profile.kind, ProfileDraftKind::Ssh);

    let mut modal_items: Vec<Element<Message>> = Vec::new();
    modal_items.push(
        row![
            text(title).size(16).color(palette.text),
            container("").width(Length::Fill),
            button_icon(
                "x",
                Message::Settings(SettingsMessage::CloseProfileModal),
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
    modal_items.push(profile_form(
        profile,
        mode,
        draft.profile_modal_base,
        draft.profile_modal_tab,
        ssh_config_profiles,
        palette,
        animations_enabled,
    ));

    if is_ssh && let Some(status) = connection_test_status_banner(test_status, palette) {
        modal_items.push(status);
    }

    modal_items.push(container("").height(Length::Fixed(8.0)).into());

    let mut footer: Vec<Element<Message>> = Vec::new();
    if is_ssh {
        let test_button: Element<Message> =
            if matches!(test_status, SshConnectionTestStatus::Testing) {
                secondary(
                    crate::t!("settings.ssh.testing"),
                    None,
                    palette,
                    animations_enabled,
                )
            } else {
                secondary(
                    crate::t!("settings.ssh.test_connection"),
                    Some(Message::Settings(SettingsMessage::TestSshConnection)),
                    palette,
                    animations_enabled,
                )
            };
        footer.push(test_button);
    }
    footer.push(container("").width(Length::Fill).into());
    footer.push(secondary(
        crate::t!("settings.ssh.cancel"),
        Some(Message::Settings(SettingsMessage::CloseProfileModal)),
        palette,
        animations_enabled,
    ));
    footer.push(primary(
        crate::t!("settings.ssh.save"),
        Message::Settings(SettingsMessage::SaveProfileModal),
        palette,
        animations_enabled,
    ));
    modal_items.push(
        row(footer)
            .spacing(SPACING_SMALL)
            .align_y(Alignment::Center)
            .width(Length::Fill)
            .into(),
    );

    let modal = container(
        column(modal_items)
            .spacing(SPACING_NORMAL)
            .padding(20)
            .width(Length::Fixed(680.0)),
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

fn backdrop(palette: Palette, progress: f32) -> container::Container<'static, Message> {
    let alpha = 0.50 * progress.clamp(0.0, 1.0);
    container(text(""))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color {
                r: palette.background.r,
                g: palette.background.g,
                b: palette.background.b,
                a: alpha,
            })),
            ..Default::default()
        })
}

fn field_label<'a>(label: &'a str, palette: Palette) -> Element<'a, Message> {
    text(label).size(11).color(palette.text_secondary).into()
}

fn hint<'a>(message: &'a str, palette: Palette) -> Element<'a, Message> {
    text(message)
        .size(10)
        .color(Color {
            a: 0.35,
            ..palette.text
        })
        .into()
}

fn tab_button<'a>(
    tab: ProfileModalTab,
    selected: bool,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let message = Message::Settings(SettingsMessage::ProfileModalTabSelected(tab));
    if selected {
        primary(tab.label(), message, palette, animations_enabled)
    } else {
        secondary(tab.label(), Some(message), palette, animations_enabled)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct BaseOption {
    index: Option<usize>,
    label: String,
}

impl std::fmt::Display for BaseOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn base_picker<'a>(
    selected: Option<usize>,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
) -> Element<'a, Message> {
    let mut options: Vec<BaseOption> = Vec::with_capacity(ssh_config_profiles.len() + 1);
    options.push(BaseOption {
        index: None,
        label: crate::t!("settings.ssh.base_manual").to_string(),
    });
    for (index, profile) in ssh_config_profiles.iter().enumerate() {
        options.push(BaseOption {
            index: Some(index),
            label: profile.name.clone(),
        });
    }
    let current = options.iter().find(|o| o.index == selected).cloned();

    column![
        field_label(crate::t!("settings.ssh.base"), palette),
        pick_list(options, current, |option: BaseOption| {
            Message::Settings(SettingsMessage::ProfileModalBaseSelected(option.index))
        })
        .width(Length::Fill)
        .text_size(13)
        .padding([6, 10])
        .style(accent_pick_list_style(palette))
        .menu_style(accent_combo_box_menu_style(palette)),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn profile_form<'a>(
    profile: &'a ProfileDraft,
    mode: ProfileModalMode,
    base_selected: Option<usize>,
    tab: ProfileModalTab,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    row![
        container(identity_column(profile, palette, animations_enabled))
            .width(Length::Fixed(210.0)),
        container(detail_column(
            profile,
            mode,
            base_selected,
            tab,
            ssh_config_profiles,
            palette,
            animations_enabled,
        ))
        .width(Length::Fill),
    ]
    .spacing(SPACING_LARGE)
    .width(Length::Fill)
    .into()
}

fn icon_picker<'a>(
    selected: &'a str,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let current = selected.trim();
    let buttons = icons::PROFILE_ICON_NAMES.iter().map(|name| {
        let active = current.eq_ignore_ascii_case(name);
        let next = if active {
            String::new()
        } else {
            (*name).to_string()
        };
        icon_toggle_content(
            icons::view(icons::by_name(name), 16.0, 1.0),
            Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                ProfileField::Icon,
                next,
            )),
            active,
            palette,
            animations_enabled,
        )
    });

    row(buttons.collect::<Vec<_>>())
        .spacing(2)
        .width(Length::Fill)
        .into()
}

fn identity_column<'a>(
    profile: &'a ProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    column![
        field_label(crate::t!("settings.ssh.display_name"), palette),
        modal_input(
            crate::t!("settings.ssh.display_name"),
            &profile.name,
            |next| {
                Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                    ProfileField::Name,
                    next,
                ))
            },
            palette,
        ),
        field_label(crate::t!("settings.ssh.profile_type"), palette),
        pick_list(
            ProfileDraftKind::ALL,
            Some(profile.kind),
            |kind: ProfileDraftKind| {
                Message::Settings(SettingsMessage::ProfileModalTypeSelected(kind))
            },
        )
        .width(Length::Fill)
        .text_size(13)
        .padding([6, 10])
        .style(accent_pick_list_style(palette))
        .menu_style(accent_combo_box_menu_style(palette)),
        field_label(crate::t!("settings.ssh.icon"), palette),
        icon_picker(&profile.icon, palette, animations_enabled),
    ]
    .spacing(4)
    .width(Length::Fill)
    .into()
}

fn detail_column<'a>(
    profile: &'a ProfileDraft,
    mode: ProfileModalMode,
    base_selected: Option<usize>,
    tab: ProfileModalTab,
    ssh_config_profiles: &'a [crate::config::SshProfile],
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'a, Message>> = Vec::new();

    match profile.kind {
        ProfileDraftKind::Local => local_fields(&mut items, profile, palette),
        ProfileDraftKind::Ssh => {
            if matches!(mode, ProfileModalMode::Create) && !ssh_config_profiles.is_empty() {
                items.push(base_picker(base_selected, ssh_config_profiles, palette));
                items.push(container("").height(Length::Fixed(4.0)).into());
            }
            items.push(
                row(ProfileModalTab::ALL
                    .into_iter()
                    .map(|candidate| {
                        tab_button(candidate, candidate == tab, palette, animations_enabled)
                    })
                    .collect::<Vec<_>>())
                .spacing(SPACING_SMALL)
                .width(Length::Fill)
                .into(),
            );
            match tab {
                ProfileModalTab::Connection => {
                    ssh_connection_fields(&mut items, profile, palette, animations_enabled)
                }
                ProfileModalTab::Advanced => ssh_advanced_fields(&mut items, profile, palette),
            }
        }
    }

    column(items).spacing(8).width(Length::Fill).into()
}

fn local_fields<'a>(
    items: &mut Vec<Element<'a, Message>>,
    profile: &'a ProfileDraft,
    palette: Palette,
) {
    items.push(field_label(crate::t!("settings.ssh.shell_path"), palette));
    items.push(
        modal_input(
            crate::t!("settings.ssh.shell_path"),
            &profile.program,
            |next| {
                Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                    ProfileField::Program,
                    next,
                ))
            },
            palette,
        )
        .into(),
    );
    items.push(hint(crate::t!("settings.ssh.shell_path_hint"), palette));
}

fn ssh_connection_fields<'a>(
    items: &mut Vec<Element<'a, Message>>,
    profile: &'a ProfileDraft,
    palette: Palette,
    animations_enabled: bool,
) {
    items.push(field_label(crate::t!("settings.ssh.endpoint"), palette));
    items.push(
        row![
            modal_input(
                crate::t!("settings.ssh.username"),
                &profile.user,
                |next| {
                    Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                        ProfileField::User,
                        next,
                    ))
                },
                palette
            )
            .width(Length::FillPortion(2)),
            text("@").size(13).color(palette.text_secondary),
            modal_input(
                crate::t!("settings.ssh.host"),
                &profile.host,
                |next| {
                    Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                        ProfileField::Host,
                        next,
                    ))
                },
                palette
            )
            .width(Length::FillPortion(3)),
            text(":").size(13).color(palette.text_secondary),
            modal_input(
                crate::t!("settings.ssh.port"),
                &profile.port,
                |next| {
                    Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                        ProfileField::Port,
                        next,
                    ))
                },
                palette
            )
            .width(Length::Fixed(64.0)),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .width(Length::Fill)
        .into(),
    );

    items.push(field_label(
        crate::t!("settings.ssh.authentication"),
        palette,
    ));
    items.push(
        row![
            auth_method_button(
                crate::t!("settings.ssh.key_file_label"),
                matches!(profile.auth_method, SshAuthMethod::KeyFile),
                "key_file",
                palette,
                animations_enabled,
            ),
            auth_method_button(
                crate::t!("settings.ssh.password"),
                matches!(profile.auth_method, SshAuthMethod::Password),
                "password",
                palette,
                animations_enabled,
            ),
        ]
        .spacing(SPACING_SMALL)
        .width(Length::Fill)
        .into(),
    );

    if matches!(profile.auth_method, SshAuthMethod::KeyFile) {
        items.push(
            modal_input(
                crate::t!("settings.ssh.key_file_input_placeholder"),
                &profile.identity_file,
                |next| {
                    Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                        ProfileField::IdentityFile,
                        next,
                    ))
                },
                palette,
            )
            .into(),
        );
        items.push(hint(crate::t!("settings.ssh.auth_hint_key_file"), palette));
    } else {
        items.push(
            modal_password(
                crate::t!("settings.ssh.password"),
                &profile.password,
                palette,
            )
            .into(),
        );
        items.push(hint(crate::t!("settings.ssh.auth_hint_password"), palette));
    }
}

fn ssh_advanced_fields<'a>(
    items: &mut Vec<Element<'a, Message>>,
    profile: &'a ProfileDraft,
    palette: Palette,
) {
    items.push(
        checkbox(profile.proxy_command_enabled)
            .label(crate::t!("settings.ssh.use_proxy_command"))
            .on_toggle(|enabled| {
                Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                    ProfileField::ProxyCommandEnabled,
                    enabled.to_string(),
                ))
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
                |next| {
                    Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                        ProfileField::ProxyCommand,
                        next,
                    ))
                },
                palette,
            )
            .into(),
        );
    }
    items.push(hint(crate::t!("settings.ssh.proxy_command_hint"), palette));
}

fn auth_method_button<'a>(
    label: &'a str,
    selected: bool,
    value: &'static str,
    palette: Palette,
    animations_enabled: bool,
) -> Element<'a, Message> {
    let message = Message::Settings(SettingsMessage::ProfileModalFieldChanged(
        ProfileField::AuthMethod,
        value.into(),
    ));
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
        .on_input(move |next| {
            Message::Settings(SettingsMessage::ProfileModalFieldChanged(
                ProfileField::Password,
                next,
            ))
        })
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
