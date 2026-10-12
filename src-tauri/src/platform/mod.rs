pub mod windows;
pub mod macos;
pub mod linux;

use tauri::AppHandle;

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

/// 注册跨平台全局监听热键
pub fn spawn_global_hotkeys(
    app_handle: AppHandle,
    trigger_fn: fn(&AppHandle),
    toggle_fn: fn(&AppHandle),
    snipping_fn: fn(&AppHandle),
) {
    #[cfg(target_os = "windows")]
    {
        windows::spawn_global_hotkeys(app_handle, trigger_fn, toggle_fn, snipping_fn);
    }
    #[cfg(target_os = "macos")]
    {
        macos::spawn_global_hotkeys(app_handle, trigger_fn, toggle_fn, snipping_fn);
    }
    #[cfg(target_os = "linux")]
    {
        linux::spawn_global_hotkeys(app_handle, trigger_fn, toggle_fn, snipping_fn);
    }
}
