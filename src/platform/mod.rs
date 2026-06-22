//! Platform-specific functionality

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub mod linux;

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use linux::*;
