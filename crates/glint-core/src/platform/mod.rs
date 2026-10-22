pub mod linux;
pub mod macos;
pub mod windows;

/// 获取当前鼠标光标物理屏幕坐标
pub fn get_cursor_position() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    {
        windows::get_cursor_position()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_cursor_position()
    }
    #[cfg(target_os = "linux")]
    {
        linux::get_cursor_position()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        (200, 200)
    }
}

/// 获取当前用户操作的前台宿主应用窗口标题 (用于多维上下文环境感知)
pub fn get_active_window_title() -> String {
    #[cfg(target_os = "windows")]
    {
        windows::get_active_window_title()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_active_window_title()
    }
    #[cfg(target_os = "linux")]
    {
        linux::get_active_window_title()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        String::new()
    }
}

/// 获取前台宿主应用窗口标题 Option 包装 (空字符串返回 None)
pub fn get_active_window_title_opt() -> Option<String> {
    let title = get_active_window_title();
    let trimmed = title.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// 跨平台获取系统剪贴板更新序列号 (Windows 原生硬件支持，非 Windows 返回 0)
pub fn get_clipboard_sequence() -> u32 {
    #[cfg(target_os = "windows")]
    {
        windows::get_clipboard_sequence()
    }
    #[cfg(not(target_os = "windows"))]
    {
        0
    }
}

/// 跨平台模拟复制操作
pub fn simulate_copy() {
    #[cfg(target_os = "windows")]
    {
        windows::simulate_copy();
    }
    #[cfg(target_os = "macos")]
    {
        macos::simulate_copy();
    }
    #[cfg(target_os = "linux")]
    {
        linux::simulate_copy();
    }
}
