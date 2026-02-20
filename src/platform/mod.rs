//! Platform-specific functionality

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

/// Placeholder for non-Windows/macOS platforms
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
#[allow(dead_code)]
pub fn apply_style(_handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    // No-op on other platforms
}
