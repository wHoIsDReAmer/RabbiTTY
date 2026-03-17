use crate::gui::app::Message;
use crate::gui::settings::{SettingsDraft, SshProfileDraft, SshProfileField, hint_text};
use crate::gui::theme::{Palette, RADIUS_SMALL, SPACING_NORMAL, SPACING_SMALL};
use iced::widget::{column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::gui::components::{button_primary, button_secondary};

const SSH_LABEL_WIDTH: f32 = 120.0;

pub fn view(draft: &SettingsDraft) -> Element<'_, Message> {
    let mut items: Vec<Element<Message>> = Vec::new();

    for (i, profile) in draft.ssh_profiles.iter().enumerate() {
        items.push(profile_card(i, profile));
    }

    if draft.ssh_profiles.is_empty() {
        items.push(hint_text(
            "No SSH profiles configured. Add one to get started.",
        ));
    }

    items.push(
        row![
            button_primary("Add Profile").on_press(Message::AddSshProfile),
            button_primary("Save").on_press(Message::SaveSshProfiles),
        ]
        .spacing(SPACING_SMALL)
        .into(),
    );

    column(items)
        .spacing(SPACING_NORMAL)
        .width(Length::Fill)
        .into()
}

fn profile_card<'a>(index: usize, profile: &'a SshProfileDraft) -> Element<'a, Message> {
    let palette = Palette::DARK;
    let title = format!("Profile #{}", index + 1);
    container(
        column(vec![
            text(title).size(14).color(palette.accent).into(),
            container("")
                .width(Length::Fill)
                .height(Length::Fixed(1.0))
                .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.15,
                        ..palette.accent
                    })),
                    ..Default::default()
                })
                .into(),
            ssh_input("Name", &profile.name, index, SshProfileField::Name),
            ssh_input("Host", &profile.host, index, SshProfileField::Host),
            ssh_input("Port", &profile.port, index, SshProfileField::Port),
            ssh_input("User", &profile.user, index, SshProfileField::User),
            ssh_input(
                "Identity File",
                &profile.identity_file,
                index,
                SshProfileField::IdentityFile,
            ),
            hint_text("Identity file supports ~ expansion (e.g. ~/.ssh/id_rsa)"),
            button_secondary("Remove")
                .on_press(Message::RemoveSshProfile(index))
                .into(),
        ])
        .spacing(SPACING_NORMAL)
        .width(Length::Fill),
    )
    .padding([16, 16])
    .width(Length::Fill)
    .style(move |_theme: &iced::Theme| iced::widget::container::Style {
        background: Some(Background::Color(Color {
            a: 0.18,
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

fn ssh_input<'a>(
    label: &'a str,
    value: &'a str,
    index: usize,
    field: SshProfileField,
) -> Element<'a, Message> {
    let palette = Palette::DARK;
    row![
        text(label).size(13).width(Length::Fixed(SSH_LABEL_WIDTH)),
        text_input("", value)
            .on_input(move |next| Message::SshProfileFieldChanged(index, field, next))
            .padding([6, 10])
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme, status: text_input::Status| {
                let focused = matches!(status, text_input::Status::Focused { .. });
                text_input::Style {
                    background: Background::Color(Color {
                        a: 0.35,
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
                                a: 0.12,
                                ..palette.text
                            }
                        },
                    },
                    icon: palette.text_secondary,
                    placeholder: palette.text_secondary,
                    value: palette.text,
                    selection: Color {
                        a: 0.3,
                        ..palette.accent
                    },
                }
            }),
    ]
    .align_y(Alignment::Center)
    .spacing(SPACING_NORMAL)
    .width(Length::Fill)
    .into()
}
