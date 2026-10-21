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
/// Uses `_NET_ACTIVE_WINDOW` + `WM_CLASS` through `xprop`. It must be
/// non-interactive: KWin's `org.kde.KWin.queryWindowInfo` looks equivalent but
/// enters "click a window" mode, turns the cursor into a crosshair and waits
/// for the user. A background companion must never steal input like that.
#[cfg(target_os = "linux")]
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    if let Some(identity) = query_active_x11_identity() {
        return identity;
    }

    // Last resort: xdotool can only give a title, so the class is guessed.
    // Must not call `get_active_window_title`, which delegates back here.
    let title = read_active_window_title().unwrap_or_default();
    let app_class = derive_app_class_from_title(&title);
    glint_core::HostIdentity::new(app_class, Some(title).filter(|t| !t.is_empty()))
}

#[cfg(target_os = "linux")]
fn query_active_x11_identity() -> Option<glint_core::HostIdentity> {
    let root = Command::new("xprop").args(["-root", "_NET_ACTIVE_WINDOW"]).output().ok()?;
    if !root.status.success() {
        return None;
    }
    let root_text = String::from_utf8_lossy(&root.stdout);
    // `_NET_ACTIVE_WINDOW(WINDOW): window id # 0x2a00005`
    let window_id = root_text.split('#').nth(1)?.trim();
    if window_id.is_empty() || window_id == "0x0" {
        return None;
    }

    let props = Command::new("xprop")
        .args(["-id", window_id, "WM_CLASS", "_NET_WM_NAME", "WM_NAME"])
        .output()
        .ok()?;
    if !props.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&props.stdout);

    // WM_CLASS is `"instance", "Class"`; the second value is the stable id.
    let class = parse_wm_class(&text).unwrap_or_default();
    let caption = parse_xprop_string(&text, "_NET_WM_NAME")
        .or_else(|| parse_xprop_string(&text, "WM_NAME"));

    if caption.is_none() && class.is_empty() {
        return None;
    }
    if is_glint_window(&class) {
        return None;
    }

    Some(glint_core::HostIdentity::new(class, caption))
}

/// Extract the class (second value) from a `WM_CLASS(STRING) = "a", "b"` line.
fn parse_wm_class(text: &str) -> Option<String> {
    let line = text.lines().find(|l| l.starts_with("WM_CLASS"))?;
    let (_, values) = line.split_once('=')?;
    let parts: Vec<&str> = values.split(',').map(|p| p.trim().trim_matches('"')).collect();
    let class = parts.get(1).or_else(|| parts.first())?.trim();
    (!class.is_empty()).then(|| class.to_string())
}

/// Extract a quoted string property such as `_NET_WM_NAME(UTF8_STRING) = "x"`.
fn parse_xprop_string(text: &str, key: &str) -> Option<String> {
    let line = text.lines().find(|l| l.starts_with(key))?;
    let (_, value) = line.split_once('=')?;
    let value = value.trim().trim_matches('"').trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// Whether a WM class belongs to Glint itself.
///
/// Matched on class only: a web page may legitimately have a title starting
/// with "Glint", but no other app reports our WM class.
fn is_glint_window(class: &str) -> bool {
    let class = class.trim().to_ascii_lowercase();
    class == "glint" || class == "com.glint.desktop" || class.ends_with(".glint")
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

/// Best-effort class from a title, for the path that cannot read WM_CLASS.
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
    fn parses_class_and_title_from_xprop() {
        let dump = "_NET_WM_NAME(UTF8_STRING) = \"Rust async book - Mozilla Firefox\"\nWM_CLASS(STRING) = \"Navigator\", \"firefox\"\n";
        assert_eq!(parse_wm_class(dump).as_deref(), Some("firefox"));
        assert_eq!(
            parse_xprop_string(dump, "_NET_WM_NAME").as_deref(),
            Some("Rust async book - Mozilla Firefox")
        );
    }

    #[test]
    fn falls_back_to_wm_name_when_utf8_name_is_absent() {
        let dump = "WM_NAME(STRING) = \"main.rs - Visual Studio Code\"\nWM_CLASS(STRING) = \"code\", \"Code\"\n";
        assert_eq!(parse_xprop_string(dump, "_NET_WM_NAME"), None);
        assert_eq!(
            parse_xprop_string(dump, "WM_NAME").as_deref(),
            Some("main.rs - Visual Studio Code")
        );
    }

    #[test]
    fn handles_a_title_containing_an_equals_sign() {
        let dump = "_NET_WM_NAME(UTF8_STRING) = \"a=b - Google Chrome\"\n";
        assert_eq!(
            parse_xprop_string(dump, "_NET_WM_NAME").as_deref(),
            Some("a=b - Google Chrome")
        );
    }

    #[test]
    fn never_treats_glint_as_the_host() {
        assert!(is_glint_window("Glint"));
        assert!(is_glint_window("com.glint.desktop"));
        assert!(!is_glint_window("google-chrome"));
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
