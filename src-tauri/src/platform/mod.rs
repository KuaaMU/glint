pub mod linux;
pub mod macos;
pub mod windows;

#[cfg(not(target_os = "linux"))]
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

/// 前台宿主应用的稳定身份：应用类别 + 标题（仅内存）。
///
/// This is what thread continuity is anchored to. The raw title is never sent
/// to the model or written to disk; see `glint_core::HostIdentity`.
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    #[cfg(target_os = "linux")]
    {
        linux::get_active_window_identity()
    }
    #[cfg(target_os = "windows")]
    {
        windows::get_active_window_identity()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_active_window_identity()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        glint_core::HostIdentity::default()
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
///
/// Linux uses the PRIMARY-selection sensor instead of clipboard polling, so
/// this API is only compiled for the platforms that still synthesise a copy.
#[cfg(not(target_os = "linux"))]
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
///
/// Linux never synthesises Ctrl+C; the PRIMARY selection is read directly.
#[cfg(not(target_os = "linux"))]
pub fn simulate_copy() {
    #[cfg(target_os = "windows")]
    {
        windows::simulate_copy();
    }
    #[cfg(target_os = "macos")]
    {
        macos::simulate_copy();
    }
}

/// 注册跨平台全局监听热键
///
/// Linux is handled by `glint_core::PlatformHotkeyEngine` in the shell setup
/// (KGlobalAccel on KDE, GlobalShortcuts portal elsewhere).
#[cfg(not(target_os = "linux"))]
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
}
