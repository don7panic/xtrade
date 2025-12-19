//! System notification helpers with platform-specific backends.
//!
//! macOS: native Notification Center via `mac-notification-sys` with an
//! AppleScript/JXA fallback to survive sandbox quirks.
//! Windows: WinRT Toast via `winrt-notification`.
//! Linux: Freedesktop notifications via `notify-rust`.
//! Other platforms: no-op.

/// Lightweight wrapper around platform notification backends.
#[derive(Clone)]
pub struct SystemNotifier {
    backend: NotificationBackend,
}

impl SystemNotifier {
    /// Create a new notifier using the provided application name label.
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            backend: NotificationBackend::new(app_name.into()),
        }
    }

    /// Fire a fire-and-forget notification. Failures are logged but do not bubble up.
    pub fn notify(&self, title: impl Into<String>, body: impl Into<String>) {
        self.backend.notify_async(title.into(), body.into());
    }
}

#[derive(Clone)]
enum NotificationBackend {
    #[cfg(target_os = "macos")]
    Mac(MacNotifier),
    #[cfg(target_os = "windows")]
    Windows(WindowsNotifier),
    #[cfg(target_os = "linux")]
    Linux(LinuxNotifier),
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    Noop,
}

impl NotificationBackend {
    fn new(app_name: String) -> Self {
        #[cfg(target_os = "macos")]
        {
            NotificationBackend::Mac(MacNotifier::new(app_name))
        }
        #[cfg(target_os = "windows")]
        {
            NotificationBackend::Windows(WindowsNotifier::new(app_name))
        }
        #[cfg(target_os = "linux")]
        {
            NotificationBackend::Linux(LinuxNotifier::new(app_name))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            NotificationBackend::Noop
        }
    }

    fn notify_async(&self, title: String, body: String) {
        match self {
            #[cfg(target_os = "macos")]
            NotificationBackend::Mac(backend) => backend.notify_async(title, body),
            #[cfg(target_os = "windows")]
            NotificationBackend::Windows(backend) => backend.notify_async(title, body),
            #[cfg(target_os = "linux")]
            NotificationBackend::Linux(backend) => backend.notify_async(title, body),
            #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
            NotificationBackend::Noop => {
                tracing::debug!("System notifications are not supported on this platform yet");
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone)]
struct MacNotifier {
    app_name: String,
}

#[cfg(target_os = "macos")]
impl MacNotifier {
    fn new(app_name: String) -> Self {
        Self { app_name }
    }

    fn notify_async(&self, title: String, body: String) {
        let app_name = self.app_name.clone();
        std::thread::spawn(move || {
            if let Err(err) = send_macos_notification(&app_name, &title, &body) {
                tracing::warn!(?err, "Failed to send macOS notification");
            }
        });
    }
}

#[cfg(target_os = "macos")]
fn send_macos_notification(app_name: &str, title: &str, body: &str) -> anyhow::Result<()> {
    use anyhow::Context;
    use mac_notification_sys::{Notification, send_notification, set_application};

    // Try native Notification Center first.
    if let Err(err) = (|| {
        if let Err(err) = set_application(app_name) {
            tracing::warn!(?err, "Failed to set macOS notification application");
        }
        send_notification(title, Some("Price alert"), body, None::<&Notification>)?;
        Ok::<_, mac_notification_sys::error::Error>(())
    })() {
        tracing::warn!(
            ?err,
            "Native macOS notification failed, falling back to osascript"
        );
        send_macos_with_osascript(app_name, title, body)
            .with_context(|| "both macOS notification paths failed")
    } else {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn escape_osascript_text(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\"', "\\\"")
}

#[cfg(target_os = "macos")]
fn escape_jxa_text(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "")
}

#[cfg(target_os = "macos")]
fn send_macos_with_osascript(app_name: &str, title: &str, body: &str) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::process::Command;

    // Prefer JXA (JavaScript for Automation) with Standard Additions, then fall back to AppleScript.
    let jxa_title = escape_jxa_text(title);
    let jxa_body = escape_jxa_text(body);
    let jxa_script = format!(
        r#"var app = Application.currentApplication();
app.includeStandardAdditions = true;
app.displayNotification("{body}", {{withTitle: "{title}", subtitle: "{app}"}});"#,
        body = jxa_body,
        title = jxa_title,
        app = escape_jxa_text(app_name),
    );

    let jxa_status = Command::new("osascript")
        .args(["-l", "JavaScript", "-e", &jxa_script])
        .status()
        .with_context(|| "failed to invoke osascript (JavaScript) for notification")?;

    if jxa_status.success() {
        return Ok(());
    }

    // Fallback to AppleScript in case JXA is unavailable or blocked.
    let escaped_title = escape_osascript_text(title);
    let escaped_body = escape_osascript_text(body);
    let script = format!(
        r#"display notification "{}" with title "{}""#,
        escaped_body, escaped_title
    );

    let status = Command::new("osascript")
        .args(["-e", &script])
        .status()
        .with_context(|| "failed to invoke osascript (AppleScript) for notification")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "osascript (AppleScript) exited with status: {:?}, JXA status: {:?}",
            status.code(),
            jxa_status.code()
        ))
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone)]
struct WindowsNotifier {
    app_id: String,
}

#[cfg(target_os = "windows")]
impl WindowsNotifier {
    fn new(app_id: String) -> Self {
        Self { app_id }
    }

    fn notify_async(&self, title: String, body: String) {
        let app_id = self.app_id.clone();
        std::thread::spawn(move || {
            if let Err(err) = send_windows_notification(&app_id, &title, &body) {
                tracing::warn!(?err, "Failed to send Windows toast");
            }
        });
    }
}

#[cfg(target_os = "windows")]
fn send_windows_notification(app_id: &str, title: &str, body: &str) -> anyhow::Result<()> {
    use anyhow::Context;
    use winrt_notification::{Duration, Toast};

    Toast::new(app_id)
        .title(title)
        .text1(body)
        .duration(Duration::Short)
        .show()
        .map(|_| ())
        .with_context(|| "failed to show Windows toast notification")
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct LinuxNotifier {
    app_name: String,
}

#[cfg(target_os = "linux")]
impl LinuxNotifier {
    fn new(app_name: String) -> Self {
        Self { app_name }
    }

    fn notify_async(&self, title: String, body: String) {
        let app_name = self.app_name.clone();
        std::thread::spawn(move || {
            if let Err(err) = send_linux_notification(&app_name, &title, &body) {
                tracing::warn!(?err, "Failed to send Linux notification");
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn send_linux_notification(app_name: &str, title: &str, body: &str) -> anyhow::Result<()> {
    use anyhow::Context;
    use notify_rust::Notification;

    Notification::new()
        .appname(app_name)
        .summary(title)
        .body(body)
        .show()
        .map(|_| ())
        .with_context(|| "failed to show Linux desktop notification")
}
