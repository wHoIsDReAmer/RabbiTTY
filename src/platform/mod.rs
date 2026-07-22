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

pub fn open_url(url: &str) {
    if !crate::terminal::url::is_openable(url) {
        eprintln!("Refusing to open non-http(s) URL: {url}");
        return;
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/c", "start", "", url]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };

    if let Err(err) = command.spawn() {
        eprintln!("Failed to open {url}: {err}");
    }
}
