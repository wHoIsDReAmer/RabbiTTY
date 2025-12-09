//! Platform-specific functionality

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::*;

/// Placeholder for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn apply_style(_handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    // No-op on other platforms
}
