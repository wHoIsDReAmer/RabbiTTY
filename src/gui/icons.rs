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

// ── UI icons ────────────────────────────────────────────────────────────────

macro_rules! lucide {
    ($konst:ident, $file:literal) => {
        static $konst: LazyLock<svg::Handle> = LazyLock::new(|| {
            svg::Handle::from_memory(include_bytes!(concat!("../../assets/icons/", $file)))
        });
    };
}

lucide!(UI_CLOSE, "lucide-x.svg");
lucide!(UI_MINIMIZE, "lucide-minus.svg");
lucide!(UI_MAXIMIZE, "lucide-square.svg");
lucide!(UI_SETTINGS, "lucide-settings.svg");
lucide!(UI_CHECK, "lucide-check.svg");
lucide!(UI_BACK, "lucide-arrow-left.svg");
lucide!(UI_TRANSFER, "lucide-arrow-left-right.svg");
lucide!(UI_TERMINAL, "lucide-terminal.svg");
lucide!(UI_FOLDER_OPEN, "lucide-folder-open.svg");
lucide!(UI_PLUGIN, "lucide-puzzle.svg");
lucide!(UI_REFRESH, "lucide-refresh-cw.svg");
lucide!(UI_PLUG, "lucide-plug.svg");
lucide!(UI_PALETTE, "lucide-palette.svg");
lucide!(UI_THEME, "lucide-sun-moon.svg");
lucide!(UI_KEYBOARD, "lucide-keyboard.svg");
lucide!(UI_SERVER, "lucide-server.svg");
lucide!(UI_ADD, "lucide-plus.svg");
lucide!(UI_UPLOAD, "lucide-upload.svg");
lucide!(UI_SFTP, "lucide-arrow-down-up.svg");
lucide!(UI_LAUNCH, "lucide-play.svg");
lucide!(UI_EDIT, "lucide-pencil.svg");
lucide!(UI_DIRECTORY, "lucide-chevron-right.svg");
lucide!(UI_SYMLINK, "lucide-link.svg");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ui {
    Close,
    Minimize,
    Maximize,
    Settings,
    Check,
    Back,
    Transfer,
    Terminal,
    FolderOpen,
    Plugin,
    Refresh,
    Plug,
    Palette,
    Theme,
    Keyboard,
    Server,
    Add,
    Upload,
    Sftp,
    Launch,
    Edit,
    Directory,
    Symlink,
}

/// Every variant, so tests can prove each one resolves to embedded bytes.
pub const ALL_UI: [Ui; 23] = [
    Ui::Close,
    Ui::Minimize,
    Ui::Maximize,
    Ui::Settings,
    Ui::Check,
    Ui::Back,
    Ui::Transfer,
    Ui::Terminal,
    Ui::FolderOpen,
    Ui::Plugin,
    Ui::Refresh,
    Ui::Plug,
    Ui::Palette,
    Ui::Theme,
    Ui::Keyboard,
    Ui::Server,
    Ui::Add,
    Ui::Upload,
    Ui::Sftp,
    Ui::Launch,
    Ui::Edit,
    Ui::Directory,
    Ui::Symlink,
];

impl Ui {
    fn handle(self) -> svg::Handle {
        match self {
            Self::Close => UI_CLOSE.clone(),
            Self::Minimize => UI_MINIMIZE.clone(),
            Self::Maximize => UI_MAXIMIZE.clone(),
            Self::Settings => UI_SETTINGS.clone(),
            Self::Check => UI_CHECK.clone(),
            Self::Back => UI_BACK.clone(),
            Self::Transfer => UI_TRANSFER.clone(),
            Self::Terminal => UI_TERMINAL.clone(),
            Self::FolderOpen => UI_FOLDER_OPEN.clone(),
            Self::Plugin => UI_PLUGIN.clone(),
            Self::Refresh => UI_REFRESH.clone(),
            Self::Plug => UI_PLUG.clone(),
            Self::Palette => UI_PALETTE.clone(),
            Self::Theme => UI_THEME.clone(),
            Self::Keyboard => UI_KEYBOARD.clone(),
            Self::Server => UI_SERVER.clone(),
            Self::Add => UI_ADD.clone(),
            Self::Upload => UI_UPLOAD.clone(),
            Self::Sftp => UI_SFTP.clone(),
            Self::Launch => UI_LAUNCH.clone(),
            Self::Edit => UI_EDIT.clone(),
            Self::Directory => UI_DIRECTORY.clone(),
            Self::Symlink => UI_SYMLINK.clone(),
        }
    }
}

pub fn ui<'a, Message: 'a>(icon: Ui, size: f32, color: Color) -> Element<'a, Message> {
    svg(icon.handle())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme: &iced::Theme, _status| svg::Style { color: Some(color) })
        .into()
}

/// An icon that brightens under the cursor. A button's `text_color` cannot reach
/// an svg, so the hover response has to live on the icon itself.
pub fn ui_hover<'a, Message: 'a>(
    icon: Ui,
    size: f32,
    idle: Color,
    hovered: Color,
) -> Element<'a, Message> {
    svg(icon.handle())
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_theme: &iced::Theme, status| svg::Style {
            color: Some(match status {
                svg::Status::Hovered => hovered,
                _ => idle,
            }),
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_ui_icon_has_an_asset() {
        for icon in ALL_UI {
            // lazy prased
            let _ = icon.handle();
        }
    }

    #[test]
    fn every_picker_name_resolves_to_a_distinct_icon() {
        let mut seen: Vec<(String, iced::Color)> = Vec::new();
        for name in PROFILE_ICON_NAMES {
            let icon = by_name(name);
            assert!(
                !seen.iter().any(|(n, c)| n != name && *c == icon.color),
                "{name} shares a color with another picker icon"
            );
            seen.push((name.to_string(), icon.color));
        }
    }

    #[test]
    fn unknown_names_fall_back_to_the_terminal_icon() {
        let fallback = by_name("something-nobody-ships");
        let terminal = by_name("terminal");
        assert_eq!(fallback.color, terminal.color);
    }
}
