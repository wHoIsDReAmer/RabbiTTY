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

/// Reveal a directory in the platform file manager. Separate from [`open_url`],
/// which only accepts http(s) and would reject every path.
pub fn open_path(path: &std::path::Path) {
    if let Err(err) = std::fs::create_dir_all(path) {
        eprintln!("Failed to create {}: {err}", path.display());
        return;
    }

    if cfg!(test) {
        return;
    }

    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let program = "xdg-open";

    // Windows `explorer` reports a non-zero exit even when it succeeds, so the
    // status is deliberately not checked on any platform.
    if let Err(err) = std::process::Command::new(program).arg(path).spawn() {
        eprintln!("Failed to open {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_path_creates_a_missing_directory() {
        let dir = std::env::temp_dir().join(format!("rabbitty-open-path-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!dir.exists());

        // Under `cfg!(test)` this returns before spawning a file manager, so the
        // only observable effect is the directory it makes first.
        open_path(&dir);

        assert!(
            dir.is_dir(),
            "the plugin folder should be created on demand"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
