//! Windows-specific window customization using DWM API

#[cfg(target_os = "windows")]
use iced::window::raw_window_handle::RawWindowHandle;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Dwm::{
    DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
    DwmSetWindowAttribute,
};

/// Apply Windows 11 style to an undecorated window:
/// - Rounded corners
/// - Dark mode titlebar (for consistency)
#[cfg(target_os = "windows")]
pub fn apply_windows_style(handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    let raw_handle = handle.as_raw();

    if let RawWindowHandle::Win32(win32_handle) = raw_handle {
        let hwnd = HWND(win32_handle.hwnd.get() as *mut _);

        unsafe {
            // Enable rounded corners (Windows 11+)
            let corner_preference = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_preference as *const _ as *const _,
                std::mem::size_of_val(&corner_preference) as u32,
            );

            // Enable dark mode for title bar elements
            let dark_mode: u32 = 1;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                &dark_mode as *const _ as *const _,
                std::mem::size_of_val(&dark_mode) as u32,
            );
        }
    }
}

/// Placeholder for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn apply_windows_style(_handle: iced::window::raw_window_handle::WindowHandle<'_>) {
    // No-op on other platforms
}
