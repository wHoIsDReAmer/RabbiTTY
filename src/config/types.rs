use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SshAuthMethod {
    KeyFile,
    #[default]
    Password,
}

/// Visual shape of the terminal text cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CursorShape {
    #[default]
    Block,
    Bar,
    Underline,
}

impl CursorShape {
    pub const ALL: [Self; 3] = [Self::Block, Self::Bar, Self::Underline];
}

impl std::fmt::Display for CursorShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Block => crate::t!("settings.terminal.cursor_shape.block"),
            Self::Bar => crate::t!("settings.terminal.cursor_shape.bar"),
            Self::Underline => crate::t!("settings.terminal.cursor_shape.underline"),
        };
        f.write_str(label)
    }
}

/// Behavior when the terminal receives a bell (`\a`, 0x07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BellMode {
    Off,
    Visual,
    #[default]
    Sound,
}

impl BellMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Visual, Self::Sound];
}

impl std::fmt::Display for BellMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Off => crate::t!("settings.terminal.bell_mode.off"),
            Self::Visual => crate::t!("settings.terminal.bell_mode.visual"),
            Self::Sound => crate::t!("settings.terminal.bell_mode.sound"),
        };
        f.write_str(label)
    }
}

/// Where the tab bar (which doubles as the title bar) is anchored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBarPosition {
    #[default]
    Top,
    Bottom,
}

impl TabBarPosition {
    pub const ALL: [Self; 2] = [Self::Top, Self::Bottom];
}

/// Action taken when the terminal area is right-clicked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RightClickAction {
    #[default]
    Paste,
    Menu,
    None,
}

impl RightClickAction {
    pub const ALL: [Self; 3] = [Self::Paste, Self::Menu, Self::None];
}

impl std::fmt::Display for RightClickAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Paste => crate::t!("settings.terminal.right_click_action.paste"),
            Self::Menu => crate::t!("settings.terminal.right_click_action.menu"),
            Self::None => crate::t!("settings.terminal.right_click_action.none"),
        };
        f.write_str(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SshProfile {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: SshAuthMethod,
    pub identity_file: Option<String>,
    #[serde(skip)]
    pub password: Option<String>,
    pub proxy_command: Option<String>,
}

impl Default for SshProfile {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            port: 22,
            user: String::new(),
            auth_method: SshAuthMethod::Password,
            identity_file: None,
            password: None,
            proxy_command: None,
        }
    }
}
