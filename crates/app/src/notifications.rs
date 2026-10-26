//! Best-effort desktop delivery: notification failures never change rename results.
#[cfg(windows)]
const APP_ID: &str = "MediaManager.Desktop";

#[cfg(windows)]
fn register() -> anyhow::Result<()> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(format!(r"Software\Classes\AppUserModelId\{APP_ID}"))?;
    key.set_value("DisplayName", &"Media Manager")?;
    key.set_value("ShowInSettings", &1u32)?;
    let id: Vec<u16> = APP_ID.encode_utf16().chain(Some(0)).collect();
    // The string remains valid for the duration of this native call.
    let result = unsafe {
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(id.as_ptr())
    };
    anyhow::ensure!(
        result >= 0,
        "Could not set notification app identity: {result}"
    );
    Ok(())
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn show(title: &str, body: &str) -> anyhow::Result<()> {
    #[cfg(windows)]
    register()?;
    let mut notification = notify_rust::Notification::new();
    notification
        .appname("Media Manager")
        .summary(title)
        .body(body);
    #[cfg(windows)]
    notification.app_id(APP_ID);
    notification.show()?;
    Ok(())
}

pub fn completed(body: String) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    std::thread::spawn(move || {
        if let Err(error) = show("Rename finished", &body) {
            eprintln!("System notification unavailable: {error:#}");
        }
    });
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let _ = body; // Native notification and background-job bridges are not installed yet.
}

#[cfg(all(test, not(any(target_os = "android", target_os = "ios"))))]
mod tests {
    #[test]
    #[ignore = "sends a real system notification; run in an interactive desktop session"]
    fn desktop_notification_smoke() {
        super::show(
            "Media Manager",
            "Rename completion notifications are ready.",
        )
        .unwrap();
    }
}
