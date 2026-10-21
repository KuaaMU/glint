#[cfg(target_os = "windows")]
use std::thread::sleep;
#[cfg(target_os = "windows")]
use std::time::Duration;
#[cfg(target_os = "windows")]
use tauri::AppHandle;

#[cfg(target_os = "windows")]
pub fn get_cursor_position() -> (i32, i32) {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
    unsafe {
        let mut pt = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut pt) != 0 {
            (pt.x, pt.y)
        } else {
            (200, 200)
        }
    }
}

#[cfg(target_os = "windows")]
pub fn get_active_window_title() -> String {
    unsafe {
        let hwnd = foreground_window();
        if hwnd.is_null() {
            return String::new();
        }
        window_text(hwnd)
    }
}

/// Foreground window's title plus the owning executable name as `app_class`.
///
/// The executable name (`chrome`, `Code`, `firefox`) is stable across window
/// title changes and is not user content, which is exactly what thread
/// continuity needs.
#[cfg(target_os = "windows")]
pub fn get_active_window_identity() -> glint_core::HostIdentity {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
        QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return glint_core::HostIdentity::default();
        }

        let title = window_text(hwnd);

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return glint_core::HostIdentity::new("", Some(title).filter(|t| !t.is_empty()));
        }

        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return glint_core::HostIdentity::new("", Some(title).filter(|t| !t.is_empty()));
        }

        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok =
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(process);

        let app_class = if ok != 0 {
            let path = String::from_utf16_lossy(&buf[..len as usize]);
            path.rsplit(['\\', '/'])
                .next()
                .unwrap_or("")
                .trim_end_matches(".exe")
                .to_ascii_lowercase()
        } else {
            String::new()
        };

        if is_glint_process(&app_class) {
            return glint_core::HostIdentity::default();
        }

        glint_core::HostIdentity::new(app_class, Some(title).filter(|t| !t.is_empty()))
    }
}

#[cfg(target_os = "windows")]
unsafe fn foreground_window() -> windows_sys::Win32::Foundation::HWND {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    unsafe { GetForegroundWindow() }
}

#[cfg(target_os = "windows")]
unsafe fn window_text(hwnd: windows_sys::Win32::Foundation::HWND) -> String {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    let mut buf = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), 512) };
    if len > 0 {
        String::from_utf16_lossy(&buf[..len as usize])
            .trim()
            .to_string()
    } else {
        String::new()
    }
}

#[cfg(target_os = "windows")]
fn is_glint_process(exe: &str) -> bool {
    let exe = exe.trim().to_ascii_lowercase();
    exe == "glint" || exe == "glint.exe"
}

#[cfg(target_os = "windows")]
unsafe extern "system" {
    pub fn GetClipboardSequenceNumber() -> u32;
}

#[cfg(target_os = "windows")]
pub fn get_clipboard_sequence() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}

#[cfg(target_os = "windows")]
pub fn simulate_copy() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, KEYEVENTF_KEYUP, VK_CONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RMENU,
        VK_RSHIFT, VK_SHIFT, keybd_event,
    };
    const VK_C: u8 = 0x43;

    unsafe {
        // 1. 等待用户手指物理松开 Alt 键 (避免目标软件判定为 Ctrl + Alt + C)
        let start = std::time::Instant::now();
        loop {
            let alt_pressed = (GetAsyncKeyState(VK_MENU as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_LMENU as i32) as u16 & 0x8000) != 0
                || (GetAsyncKeyState(VK_RMENU as i32) as u16 & 0x8000) != 0;
            if !alt_pressed {
                break;
            }
            sleep(Duration::from_millis(8));
            if start.elapsed().as_millis() > 300 {
                break;
            }
        }

        // 2. 彻底释放全部修饰键 (Alt, Control, Shift)
        keybd_event(VK_LMENU as u8, 0x38, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RMENU as u8, 0x38, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_MENU as u8, 0x38, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_LSHIFT as u8, 0x2A, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_RSHIFT as u8, 0x36, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_SHIFT as u8, 0x2A, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0x1D, KEYEVENTF_KEYUP, 0);

        // 短暂休眠 25ms，让目标软件窗口的消息泵处理按键释放
        sleep(Duration::from_millis(25));

        // 3. 发送 Ctrl + C (带硬件扫描码 0x1D 和 0x2E，兼顾各类现代浏览器、Electron 与终端)
        keybd_event(VK_CONTROL as u8, 0x1D, 0, 0);
        keybd_event(VK_C, 0x2E, 0, 0);
        sleep(Duration::from_millis(20));

        // 4. 释放 C 与 Ctrl
        keybd_event(VK_C, 0x2E, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0x1D, KEYEVENTF_KEYUP, 0);
    }
}

#[cfg(target_os = "windows")]
pub fn spawn_global_hotkeys(
    app_handle: AppHandle,
    trigger_fn: fn(&AppHandle),
    toggle_fn: fn(&AppHandle),
    snipping_fn: fn(&AppHandle),
) {
    std::thread::spawn(move || {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            MOD_ALT, MOD_NOREPEAT, RegisterHotKey,
        };
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

        unsafe {
            // 全局热键 Alt + Q (VK_Q = 0x51) -> 划词透视
            let _ = RegisterHotKey(
                std::ptr::null_mut(),
                101,
                (MOD_ALT | MOD_NOREPEAT) as u32,
                0x51,
            );
            // 全局热键 Alt + D (VK_D = 0x44) -> 快速透视唤出/显隐切换
            let _ = RegisterHotKey(
                std::ptr::null_mut(),
                102,
                (MOD_ALT | MOD_NOREPEAT) as u32,
                0x44,
            );
            // 全局热键 Alt + W (VK_W = 0x57) -> 截屏透视与标注
            let _ = RegisterHotKey(
                std::ptr::null_mut(),
                103,
                (MOD_ALT | MOD_NOREPEAT) as u32,
                0x57,
            );

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) != 0 {
                if msg.message == WM_HOTKEY {
                    match msg.wParam {
                        101 => trigger_fn(&app_handle),
                        102 => toggle_fn(&app_handle),
                        103 => snipping_fn(&app_handle),
                        _ => {}
                    }
                }
            }
        }
    });
}
