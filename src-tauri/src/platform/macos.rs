#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::thread::sleep;
#[cfg(target_os = "macos")]
use std::time::Duration;
use tauri::AppHandle;

#[cfg(target_os = "macos")]
pub fn get_cursor_position() -> (i32, i32) {
    // macOS AppleScript 动态提取鼠标全局物理位置
    let output = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to return {x, y} of (get mouse location)",
        ])
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout);
        let parts: Vec<&str> = text.trim().split(',').map(|s| s.trim()).collect();
        if parts.len() == 2 {
            if let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                return (x, y);
            }
        }
    }
    (300, 200)
}

#[cfg(target_os = "macos")]
pub fn get_active_window_title() -> String {
    get_active_window_identity().title.unwrap_or_default()
}

/// Frontmost app and its window title.
///
/// The bundle identifier (`com.google.Chrome`, `com.microsoft.VSCode`) is a
/// stable, non-content app identity — exactly what thread continuity needs.
/// The title is user content and stays in memory only.
#[cfg(target_os = "macos")]
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    // Tab-separated so a title containing " - " cannot confuse the split.
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            set appId to bundle identifier of frontApp
            if appId is missing value then set appId to name of frontApp
            try
                set winTitle to name of front window of frontApp
                return appId & tab & winTitle
            on error
                return appId & tab
            end try
        end tell
    "#;
    let output = Command::new("osascript").args(["-e", script]).output();

    let Ok(out) = output else {
        return glint_core::HostIdentity::default();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.trim().splitn(2, '\t');
    let app_id = parts.next().unwrap_or("").trim().to_string();
    let title = parts.next().unwrap_or("").trim().to_string();

    if is_glint_app(&app_id) {
        return glint_core::HostIdentity::default();
    }

    glint_core::HostIdentity::new(app_id, (!title.is_empty()).then_some(title))
}

#[cfg(target_os = "macos")]
fn is_glint_app(app_id: &str) -> bool {
    let app_id = app_id.trim().to_ascii_lowercase();
    app_id == "glint" || app_id == "com.glint.desktop" || app_id.ends_with(".glint")
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn get_active_window_title() -> String {
    String::new()
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn get_cursor_position() -> (i32, i32) {
    (300, 200)
}

#[cfg(target_os = "macos")]
pub fn simulate_copy() {
    // macOS: 通过 System Events 模拟 Cmd + C
    let _ = Command::new("osascript")
        .args([
            "-e",
            "tell application \"System Events\" to keystroke \"c\" using command down",
        ])
        .output();
    sleep(Duration::from_millis(20));
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn simulate_copy() {}

#[cfg(target_os = "macos")]
pub fn spawn_global_hotkeys(
    _app_handle: AppHandle,
    _trigger_fn: fn(&AppHandle),
    _toggle_fn: fn(&AppHandle),
    _snipping_fn: fn(&AppHandle),
) {
    // macOS 全局热键挂载 (Cmd+Option+Q, Cmd+Option+D, Cmd+Option+W)
    println!("[macOS] Glint hotkeys initialized for macOS");
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub fn spawn_global_hotkeys(
    _app_handle: AppHandle,
    _trigger_fn: fn(&AppHandle),
    _toggle_fn: fn(&AppHandle),
    _snipping_fn: fn(&AppHandle),
) {
}
