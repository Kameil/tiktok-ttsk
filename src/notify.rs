use tracing::warn;

pub fn send(title: &str, message: &str) {
    #[cfg(not(target_os = "android"))]
    {
        if let Err(e) = notify_rust::Notification::new()
            .summary(title)
            .body(message)
            .appname("tiktok-ttsk")
            .show()
        {
            warn!("failed to send desktop notification: {e}");
        }
    }
    #[cfg(target_os = "android")]
    {
        let _ = (title, message);
    }
}
