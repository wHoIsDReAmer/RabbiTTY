pub const DEFAULT_WINDOW_WIDTH: f32 = 600.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 350.0;
pub const FONT_SCALE_FACTOR: f32 = 0.85;
pub const DEFAULT_THEME_FOREGROUND: [u8; 3] = [0xcd, 0xd6, 0xf4];
pub const DEFAULT_THEME_BACKGROUND: [u8; 3] = [0x1e, 0x1e, 0x2e];
pub const DEFAULT_THEME_CURSOR: [u8; 3] = [0x89, 0xb4, 0xfa];
pub const DEFAULT_THEME_BG_OPACITY: f32 = 1.0;
pub const DEFAULT_BLUR_ENABLED: bool = true;
pub const DEFAULT_MACOS_BLUR_RADIUS: i32 = 20;

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_NEW_TAB: &str = "Command+T";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_NEW_TAB: &str = "Ctrl+T";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_CLOSE_TAB: &str = "Command+W";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_CLOSE_TAB: &str = "Ctrl+W";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_OPEN_SETTINGS: &str = "Command+Comma";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_OPEN_SETTINGS: &str = "Ctrl+Comma";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_NEXT_TAB: &str = "Command+PageDown";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_NEXT_TAB: &str = "Ctrl+PageDown";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_PREV_TAB: &str = "Command+PageUp";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_PREV_TAB: &str = "Ctrl+PageUp";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_QUIT: &str = "Command+Q";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_QUIT: &str = "Ctrl+Q";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_INCREASE: &str = "Command+=";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_INCREASE: &str = "Ctrl+=";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_DECREASE: &str = "Command+-";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_DECREASE: &str = "Ctrl+-";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_FONT_SIZE_RESET: &str = "Command+0";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_FONT_SIZE_RESET: &str = "Ctrl+0";

#[cfg(target_os = "macos")]
pub const DEFAULT_SHORTCUT_DUPLICATE_TAB: &str = "Command+D";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SHORTCUT_DUPLICATE_TAB: &str = "Ctrl+Shift+D";

pub const DEFAULT_TERMINAL_FONT_SIZE: f32 = 14.0;
pub const DEFAULT_TERMINAL_PADDING_X: f32 = 4.0;
pub const DEFAULT_TERMINAL_PADDING_Y: f32 = 4.0;
pub const DEFAULT_TERMINAL_SCROLLBACK: usize = 10_000;
pub const DEFAULT_BRACKETED_PASTE: bool = true;
pub const DEFAULT_MULTILINE_PASTE_CONFIRM: bool = false;
pub const DEFAULT_TERMINAL_SCROLL_MULTIPLIER: f32 = 1.0;
pub const DEFAULT_CURSOR_BLINK: bool = true;
pub const DEFAULT_BOLD_IS_BRIGHT: bool = false;
pub const DEFAULT_ANIMATIONS_ENABLED: bool = true;
