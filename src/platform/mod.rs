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

pub fn notify(title: &str, body: &str) {
    if cfg!(test) {
        return;
    }

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("osascript");
        c.args([
            "-e",
            "on run argv",
            "-e",
            "display notification (item 1 of argv) with title (item 2 of argv)",
            "-e",
            "end run",
            "--",
        ]);
        c.args([body, title]);
        c
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("powershell");
        c.args(["-NoProfile", "-NonInteractive", "-Command", WINDOWS_TOAST]);
        c.env("RABBITTY_NOTIFY_TITLE", title);
        c.env("RABBITTY_NOTIFY_BODY", body);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut c = std::process::Command::new("notify-send");
        c.args(["--app-name=Rabbitty", "--", title, body]);
        c
    };

    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(err) => eprintln!("Failed to post a notification: {err}"),
    }
}

#[cfg(target_os = "windows")]
const WINDOWS_TOAST: &str = r#"
[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType=WindowsRuntime] | Out-Null
$template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02)
$texts = $template.GetElementsByTagName('text')
$texts.Item(0).AppendChild($template.CreateTextNode($env:RABBITTY_NOTIFY_TITLE)) | Out-Null
$texts.Item(1).AppendChild($template.CreateTextNode($env:RABBITTY_NOTIFY_BODY)) | Out-Null
$toast = [Windows.UI.Notifications.ToastNotification]::new($template)
[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Rabbitty').Show($toast)
"#;

pub fn open_url(url: &str) {
    if !crate::terminal::url::is_openable(url) {
        eprintln!("Refusing to open non-http(s) URL: {url}");
        return;
    }

    if cfg!(test) {
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
