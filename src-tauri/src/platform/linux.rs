use std::process::Command;

#[cfg(target_os = "linux")]
pub fn get_cursor_position() -> (i32, i32) {
    if let Ok(out) = Command::new("xdotool").arg("getmouselocation").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        let mut x = 300;
        let mut y = 200;
        for part in text.split_whitespace() {
            if let Some(val) = part.strip_prefix("x:") {
                if let Ok(v) = val.parse::<i32>() {
                    x = v;
                }
            } else if let Some(val) = part.strip_prefix("y:") {
                if let Ok(v) = val.parse::<i32>() {
                    y = v;
                }
            }
        }
        return (x, y);
    }
    (300, 200)
}

/// Read the frontmost window's title and application class.
///
/// KWin's D-Bus interface gives both `caption` (title) and `resourceClass`
/// (stable app id, e.g. `firefox`, `code`). `xdotool` is the X11 fallback and
/// only yields a title, so the class is derived from it.
#[cfg(target_os = "linux")]
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    if let Some(identity) = query_kwin_identity() {
        return identity;
    }

    // X11 fallback: no WM_CLASS read here, only a title, so the class is
    // guessed. Must not call `get_active_window_title`, which delegates back
    // into this function.
    let title = read_active_window_title().unwrap_or_default();
    let app_class = derive_app_class_from_title(&title);
    glint_core::HostIdentity::new(app_class, Some(title).filter(|t| !t.is_empty()))
}

#[cfg(target_os = "linux")]
fn read_active_window_title() -> Option<String> {
    let output = Command::new("xdotool")
        .args(["getactivewindow", "getwindowname"])
        .output()
        .ok()?;
    let title = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!title.is_empty()).then_some(title)
}

#[cfg(target_os = "linux")]
fn query_kwin_identity() -> Option<glint_core::HostIdentity> {
    let output = Command::new("busctl")
        .args([
            "--user",
            "call",
            "org.kde.KWin",
            "/KWin",
            "org.kde.KWin",
            "queryWindowInfo",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let caption = dbus_string_field(&text, "caption");
    let class = dbus_string_field(&text, "resourceClass")
        .or_else(|| dbus_string_field(&text, "desktopFile"))
        .unwrap_or_default();

    if caption.is_none() && class.is_empty() {
        return None;
    }

    // Never report Glint itself as the host. The HUD (and the snipping
    // overlay) take focus right after a trigger, and anchoring a thread to
    // "Glint" would merge every unrelated selection into one document.
    if is_glint_window(&class) {
        return None;
    }

    Some(glint_core::HostIdentity::new(class, caption))
}

/// Whether a WM class belongs to Glint itself.
///
/// Matched on class only: a web page may legitimately have a title starting
/// with "Glint", but no other app reports our WM class.
fn is_glint_window(class: &str) -> bool {
    let class = class.trim().to_ascii_lowercase();
    class == "glint" || class == "com.glint.desktop" || class.ends_with(".glint")
}

/// Pull one `"key" s "value"` field out of a `busctl call` dump.
///
/// The dump is a single line of space-separated values, and titles may contain
/// quotes, so this walks the token list instead of using a regex.
#[cfg(target_os = "linux")]
fn dbus_string_field(text: &str, key: &str) -> Option<String> {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if *token != format!("\"{key}\"") {
            continue;
        }
        // `<key> s <value>`; busctl keeps the quotes and does not split inside
        // them, so the next token is the whole value.
        let value = tokens.get(i + 2)?.trim_matches('"').trim().to_string();
        return (!value.is_empty()).then_some(value);
    }
    None
}

/// Best-effort class from a title, for the X11 path that cannot read WM_CLASS.
#[cfg(target_os = "linux")]
fn derive_app_class_from_title(title: &str) -> String {
    let lower = title.to_lowercase();
    for (needle, class) in [
        ("mozilla firefox", "firefox"),
        ("google chrome", "google-chrome"),
        ("chromium", "chromium"),
        ("visual studio code", "code"),
        ("konsole", "konsole"),
        ("okular", "okular"),
        ("zathura", "zathura"),
        ("evince", "evince"),
    ] {
        if lower.contains(needle) {
            return class.to_string();
        }
    }
    String::new()
}

#[cfg(target_os = "linux")]
pub fn get_active_window_title() -> String {
    get_active_window_identity()
        .title
        .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_active_window_title() -> String {
    String::new()
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    glint_core::HostIdentity::default()
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_cursor_position() -> (i32, i32) {
    (300, 200)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn parses_caption_and_class_from_a_busctl_dump() {
        let dump = r#"a{sv} 29 "activities" as 0 "caption" s "Tokio" "clientMachine" s "localhost" "resourceClass" s "firefox" "width" d 1920"#;
        assert_eq!(dbus_string_field(dump, "caption").as_deref(), Some("Tokio"));
        assert_eq!(
            dbus_string_field(dump, "resourceClass").as_deref(),
            Some("firefox")
        );
    }

    #[test]
    fn missing_fields_return_none() {
        let dump = r#"a{sv} 1 "caption" s "x""#;
        assert_eq!(dbus_string_field(dump, "resourceClass"), None);
    }

    #[test]
    fn never_treats_glint_as_the_host() {
        assert!(is_glint_window("Glint"));
        assert!(is_glint_window("com.glint.desktop"));
        assert!(!is_glint_window("google-chrome"));
        // A page titled "Glint: ..." is still a valid host document.
        assert!(!is_glint_window("firefox"));
    }

    #[test]
    fn derives_a_class_from_a_browser_title() {
        assert_eq!(
            derive_app_class_from_title("Async in depth - Tokio - Mozilla Firefox"),
            "firefox"
        );
        assert_eq!(derive_app_class_from_title("main.rs - Visual Studio Code"), "code");
        assert_eq!(derive_app_class_from_title("anything else"), "");
    }
}

// Intentionally no `simulate_copy` or `spawn_global_hotkeys` on Linux:
// selection comes from `glint_core::platform_sensor()` and hotkeys from
// `glint_core::PlatformHotkeyEngine` (KGlobalAccel / GlobalShortcuts portal).
