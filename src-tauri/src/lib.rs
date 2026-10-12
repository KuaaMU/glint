mod capture;
mod config;
mod llm;
mod platform;
mod preferences;

use capture::{capture_selected_text, get_cursor_pos, SelectionResult};
use config::load_config;
use llm::{stream_explanation, QueryRequest, StreamChunk};
use preferences::{
    fetch_models, get_app_config, hide_preferences_window, open_preferences_window,
    test_llm_connection, update_app_config,
};
use tauri::{
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
};

#[tauri::command]
fn get_selection() -> Option<SelectionResult> {
    capture_selected_text()
}

#[tauri::command]
fn get_screen_capture() -> Result<capture::ScreenCaptureResult, String> {
    capture::capture_screen()
}

#[tauri::command]
async fn explain(app: AppHandle, mut req: QueryRequest, on_event: Channel<StreamChunk>) {
    // 自动兜底：若前端传来的配置为空，从系统持久化配置 glint.config.json 自动注入
    let cfg = load_config(&app);
    if req.api_key.as_deref().unwrap_or("").trim().is_empty() && !cfg.api_key.trim().is_empty() {
        req.api_key = Some(cfg.api_key.clone());
    }
    if req.base_url.as_deref().unwrap_or("").trim().is_empty() {
        req.base_url = Some(cfg.base_url.clone());
    }
    if req.model.as_deref().unwrap_or("").trim().is_empty() {
        req.model = Some(cfg.model.clone());
    }

    stream_explanation(req, on_event).await;
}

pub fn position_hud_window(window: &WebviewWindow, cursor_x: i32, cursor_y: i32) {
    let win_size = window.outer_size().unwrap_or(PhysicalSize::new(440, 380));
    let win_w = win_size.width as i32;
    let win_h = win_size.height as i32;

    let mut target_x = cursor_x + 8;
    let mut target_y = cursor_y + 8;

    if let Ok(Some(monitor)) = window.current_monitor() {
        let mon_size = monitor.size();
        let mon_pos = monitor.position();
        let mon_x = mon_pos.x;
        let mon_y = mon_pos.y;
        let mon_w = mon_size.width as i32;
        let mon_h = mon_size.height as i32;

        if target_x + win_w > mon_x + mon_w {
            target_x = (cursor_x - win_w - 8).max(mon_x + 10);
        }
        if target_y + win_h > mon_y + mon_h {
            target_y = (cursor_y - win_h - 8).max(mon_y + 10);
        }
    } else {
        target_x = target_x.max(10);
        target_y = target_y.max(10);
    }

    let _ = window.set_position(PhysicalPosition::new(target_x, target_y));
}

#[tauri::command]
fn reposition_window(app: AppHandle, x: i32, y: i32) {
    if let Some(window) = app.get_webview_window("hud") {
        position_hud_window(&window, x, y);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn resize_hud_window(app: AppHandle, width: f64, height: f64) {
    if let Some(window) = app.get_webview_window("hud") {
        let clamped_w = width.clamp(320.0, 800.0);
        let clamped_h = height.clamp(240.0, 900.0);
        let _ = window.set_size(LogicalSize::new(clamped_w, clamped_h));
    }
}

#[tauri::command]
fn reset_hud_size(app: AppHandle) {
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.set_size(LogicalSize::new(440.0, 380.0));
    }
}

#[tauri::command]
fn hide_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("hud") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn toggle_hud_window(app: AppHandle) {
    toggle_window_visibility(&app);
}

pub fn toggle_window_visibility(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("hud") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let (cx, cy) = get_cursor_pos();
            position_hud_window(&window, cx, cy);
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

pub fn trigger_capture_and_show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("hud") {
        let (text, cursor_x, cursor_y) = match capture_selected_text() {
            Some(res) => (res.text, res.cursor_x, res.cursor_y),
            None => {
                let (cx, cy) = get_cursor_pos();
                ("".to_string(), cx, cy)
            }
        };

        position_hud_window(&window, cursor_x, cursor_y);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("glint-trigger", text);
    }
}

pub fn trigger_snipping_and_show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("snipping") {
        match capture::capture_screen() {
            Ok(cap) => {
                let _ = window.set_position(PhysicalPosition::new(cap.geometry.x, cap.geometry.y));
                let _ = window.set_size(PhysicalSize::new(cap.geometry.width, cap.geometry.height));
                let _ = window.emit("glint-snipping-capture", &cap);
                let _ = window.show();
                let _ = window.set_focus();
            }
            Err(e) => {
                eprintln!("[Glint] 截屏错误: {}", e);
            }
        }
    }
}

#[tauri::command]
fn open_snipping_tool(app: AppHandle) {
    trigger_snipping_and_show(&app);
}

#[tauri::command]
fn close_snipping_tool(app: AppHandle) {
    if let Some(window) = app.get_webview_window("snipping") {
        let _ = window.hide();
    }
}

#[tauri::command]
fn submit_snipping_result(app: AppHandle, image_data_url: String, prompt: Option<String>) {
    if let Some(snip_win) = app.get_webview_window("snipping") {
        let _ = snip_win.hide();
    }

    if let Some(hud_win) = app.get_webview_window("hud") {
        let (cx, cy) = get_cursor_pos();
        position_hud_window(&hud_win, cx, cy);
        let _ = hud_win.show();
        let _ = hud_win.set_focus();
        let _ = hud_win.emit(
            "glint-multimodal-trigger",
            serde_json::json!({
                "image_data_url": image_data_url,
                "prompt": prompt
            }),
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化窗口置顶状态
            let cfg = load_config(&app.handle());
            if let Some(hud) = app.get_webview_window("hud") {
                let _ = hud.set_always_on_top(cfg.always_on_top);
            }

            // 启动跨平台系统全局热键监听 (Windows / macOS / Linux)
            platform::spawn_global_hotkeys(
                app.handle().clone(),
                trigger_capture_and_show,
                toggle_window_visibility,
                trigger_snipping_and_show,
            );

            // 设置系统托盘 (支持左键快速显隐、右键唤起菜单，明确定制 Tooltip 与原生图标)
            let show_i = MenuItem::with_id(app, "show", "划词透视 (Alt + Q)", true, None::<&str>)?;
            let snip_i = MenuItem::with_id(app, "snip", "截屏透视 (Alt + W)", true, None::<&str>)?;
            let pref_i = MenuItem::with_id(app, "preferences", "偏好设置...", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出 Glint", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &snip_i, &pref_i, &quit_i])?;

            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Glint - 伴生透视 (Alt + Q / Alt + W)")
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click { button: tauri::tray::MouseButton::Left, .. } = event {
                        let app = tray.app_handle();
                        toggle_window_visibility(app);
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        trigger_capture_and_show(app);
                    }
                    "snip" => {
                        trigger_snipping_and_show(app);
                    }
                    "preferences" => {
                        let _ = open_preferences_window(app.clone());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                });

            if let Some(app_icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(app_icon.clone());
            }

            let _tray = tray_builder.build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "preferences" || window.label() == "snipping" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_selection,
            get_screen_capture,
            explain,
            reposition_window,
            resize_hud_window,
            reset_hud_size,
            hide_window,
            toggle_hud_window,
            open_preferences_window,
            hide_preferences_window,
            get_app_config,
            update_app_config,
            fetch_models,
            test_llm_connection,
            open_snipping_tool,
            close_snipping_tool,
            submit_snipping_result
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snipping_trigger_payload_structure() {
        let payload = serde_json::json!({
            "image_data_url": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
            "prompt": "请分析此屏幕截图区域"
        });

        assert!(payload["image_data_url"].as_str().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(payload["prompt"].as_str().unwrap(), "请分析此屏幕截图区域");
    }

    #[test]
    fn test_multimodal_multi_turn_payload_generation() {
        let req = llm::QueryRequest {
            messages: vec![
                llm::ChatMessage::user_multimodal("请分析此截屏重点", "data:image/png;base64,AAAA"),
                llm::ChatMessage::assistant("这是包含系统架构图的截屏，标注重点在于高速缓存和内存拓扑。"),
                llm::ChatMessage::user_text("其中 Cache 一致性如何避免性能倒退？"),
            ],
            api_key: Some("sk-test".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            model: Some("gpt-4o".to_string()),
        };

        let json_payload = llm::build_api_payload(&req);
        let msgs = json_payload["messages"].as_array().unwrap();
        // system prompt + 3 turns = 4 messages
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[3]["role"], "user");
        assert_eq!(msgs[3]["content"], "其中 Cache 一致性如何避免性能倒退？");
    }
}

