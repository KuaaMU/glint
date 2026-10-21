#[cfg(target_os = "linux")]
use std::process::Command;

#[cfg(target_os = "linux")]
pub fn get_cursor_position() -> (i32, i32) {
    // Linux X11 / Wayland fallback via xdotool
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

#[cfg(target_os = "linux")]
pub fn get_active_window_title() -> String {
    if let Ok(out) = Command::new("xdotool").args(["getactivewindow", "getwindowname"]).output() {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        String::new()
    }
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_active_window_title() -> String {
    String::new()
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn get_cursor_position() -> (i32, i32) {
    (300, 200)
}

// Intentionally no `simulate_copy` or `spawn_global_hotkeys` on Linux:
// selection comes from `glint_core::platform_sensor()` and hotkeys from
// `glint_core::PlatformHotkeyEngine` (KGlobalAccel / GlobalShortcuts portal).
