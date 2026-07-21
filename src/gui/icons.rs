use iced::widget::svg;
use iced::{Color, Element, Length};
use std::sync::LazyLock;

static ICON_BASH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/bash.svg")));
static ICON_ZSH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/zsh.svg")));
static ICON_FISH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/fish.svg")));
static ICON_POWERSHELL: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/powershell.svg")));
static ICON_TERMINAL: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/terminal.svg")));
static ICON_SSH: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/icons/ssh.svg")));

pub struct ShellIcon {
    pub handle: svg::Handle,
    pub color: Color,
}

/// Icon names a profile may pin, in picker order.
pub const PROFILE_ICON_NAMES: [&str; 6] = ["terminal", "bash", "zsh", "fish", "powershell", "ssh"];

pub fn by_name(name: &str) -> ShellIcon {
    match name.to_lowercase().as_str() {
        "bash" => ShellIcon {
            handle: ICON_BASH.clone(),
            color: Color::from_rgb8(0x4E, 0xAA, 0x25),
        },
        "zsh" => ShellIcon {
            handle: ICON_ZSH.clone(),
            color: Color::from_rgb8(0xF1, 0x5A, 0x24),
        },
        "fish" => ShellIcon {
            handle: ICON_FISH.clone(),
            color: Color::from_rgb8(0x34, 0xC5, 0x34),
        },
        "pwsh" | "powershell" => ShellIcon {
            handle: ICON_POWERSHELL.clone(),
            color: Color::from_rgb8(0x5A, 0x91, 0xD8),
        },
        "ssh" => ssh(),
        _ => ShellIcon {
            handle: ICON_TERMINAL.clone(),
            color: Color::from_rgb8(0x4C, 0xC2, 0xFF),
        },
    }
}

pub fn ssh() -> ShellIcon {
    ShellIcon {
        handle: ICON_SSH.clone(),
        color: Color::from_rgb8(0x4F, 0xC0, 0x8D),
    }
}

/// The shell binary the user's `$SHELL` points at, for default-shell profiles.
pub fn default_shell_name() -> String {
    let shell = std::env::var("SHELL").unwrap_or_default();
    std::path::Path::new(&shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

pub fn view<'a, Message: 'a>(icon: ShellIcon, size: f32, opacity: f32) -> Element<'a, Message> {
    let color = icon.color;
    svg(icon.handle)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .opacity(opacity)
        .style(move |_theme: &iced::Theme, _status| svg::Style { color: Some(color) })
        .into()
}
