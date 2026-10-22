#[cfg(target_os = "macos")]
use std::process::Command;
#[cfg(target_os = "macos")]
use std::thread::sleep;
#[cfg(target_os = "macos")]
use std::time::Duration;

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
        if parts.len() == 2
            && let (Ok(x), Ok(y)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>())
        {
            return (x, y);
        }
    }
    (300, 200)
}

#[cfg(target_os = "macos")]
pub fn get_active_window_title() -> String {
    let script = r#"
        tell application "System Events"
            set frontApp to first application process whose frontmost is true
            try
                set winTitle to name of front window of frontApp
                return (name of frontApp) & " - " & winTitle
            on error
                return name of frontApp
            end try
        end tell
    "#;
    let output = Command::new("osascript").args(["-e", script]).output();
    if let Ok(out) = output {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        String::new()
    }
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
