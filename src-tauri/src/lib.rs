mod capture;
mod config;
mod llm;
mod placement;
mod platform;
mod preferences;
pub mod tools;

use capture::{capture_selected_text, get_cursor_pos, SelectionResult};
use config::load_config;
use llm::{stream_explanation, QueryRequest, StreamChunk};
use preferences::{
    fetch_models, get_app_config, hide_preferences_window, open_preferences_window,
    test_llm_connection, update_app_config,
};
use std::sync::Arc;
use tauri::{
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
};

const GLINT_APP_ID: &str = "com.glint.desktop";

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

/// Place the HUD near the cursor/selection, preferring below then above so the
/// card never covers the text it explains.
///
/// Wayland ignores `set_position` on a normal toplevel, so this only takes
/// effect on X11/XWayland; the caller logs whether it actually moved.
pub fn position_hud_window(
    window: &WebviewWindow,
    cursor_x: i32,
    cursor_y: i32,
    selection: Option<placement::Rect>,
    card_w: i32,
    card_h: i32,
) {
    let monitor_rect = window
        .current_monitor()
        .ok()
        .flatten()
        .map(|monitor| {
            let size = monitor.size();
            let pos = monitor.position();
            placement::Rect {
                x: pos.x,
                y: pos.y,
                width: size.width as i32,
                height: size.height as i32,
            }
        })
        .unwrap_or(placement::Rect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        });

    let bounds = placement::SelectionBounds {
        cursor: placement::Point {
            x: cursor_x,
            y: cursor_y,
        },
        extent: selection,
    };
    let target = placement::place_near_selection(bounds, monitor_rect, card_w, card_h);

    match window.set_position(PhysicalPosition::new(target.x, target.y)) {
        Ok(()) => {}
        Err(err) => eprintln!("[Glint] set_position failed (Wayland toplevel?): {err}"),
    }

    // Verify the compositor honoured the move. Under a native Wayland toplevel
    // this is silently ignored; the card would then open wherever the
    // compositor likes (usually centre), which is exactly what we must avoid.
    // Only meaningful once mapped; callers invoke this after `show()`.
    if window.is_visible().unwrap_or(false) {
        if let Ok(actual) = window.outer_position() {
            let dx = (actual.x - target.x).abs();
            let dy = (actual.y - target.y).abs();
            if dx > 4 || dy > 4 {
                eprintln!(
                    "[Glint] placement not honoured: wanted ({}, {}), got ({}, {}); \
                     run with GDK_BACKEND=x11 for cursor-adjacent placement",
                    target.x, target.y, actual.x, actual.y
                );
            }
        }
    }
}

/// Current outer size in physical pixels, defaulting to the configured card.
fn hud_outer_size(window: &WebviewWindow) -> (i32, i32) {
    match window.outer_size() {
        Ok(size) if size.width > 0 && size.height > 0 => (size.width as i32, size.height as i32),
        _ => {
            let scale = window.scale_factor().unwrap_or(1.0);
            (
                (440.0 * scale).round() as i32,
                (380.0 * scale).round() as i32,
            )
        }
    }
}

#[tauri::command]
fn reposition_window(app: AppHandle, x: i32, y: i32) {
    if let Some(window) = app.get_webview_window("hud") {
        let (w, h) = hud_outer_size(&window);
        position_hud_window(&window, x, y, None, w, h);
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

/// Whether the user pinned the card. A pinned HUD stays visible across new
/// selections; Escape and the close button still dismiss it.
static HUD_PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
fn set_hud_pinned(pinned: bool) {
    HUD_PINNED.store(pinned, std::sync::atomic::Ordering::Relaxed);
}

fn hud_is_pinned() -> bool {
    HUD_PINNED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Resize and place the card, keeping a pinned card's user-set size.
///
/// The window must be mapped before a position can be read back, so this runs
/// after `show()`.
fn prepare_hud(window: &WebviewWindow, cursor_x: i32, cursor_y: i32) {
    if !hud_is_pinned() {
        if let Err(err) = window.set_size(LogicalSize::new(440.0, 380.0)) {
            eprintln!("[Glint] set_size failed: {err}");
        }
    }
    let (w, h) = hud_outer_size(window);
    position_hud_window(window, cursor_x, cursor_y, None, w, h);
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
            let (w, h) = hud_outer_size(&window);
            position_hud_window(&window, cx, cy, None, w, h);
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

pub fn trigger_capture_and_show(app: &AppHandle) {
    eprintln!("[Glint] trigger_capture_and_show");
    if let Some(window) = app.get_webview_window("hud") {
        eprintln!(
            "[Glint] hud window found; visible={:?} outer={:?}",
            window.is_visible(),
            window.outer_size()
        );
        let (text, cursor_x, cursor_y, window_title) = match capture_selected_text() {
            Some(res) => (res.text, res.cursor_x, res.cursor_y, res.window_title),
            None => {
                let (cx, cy) = get_cursor_pos();
                let window_title = crate::platform::get_active_window_title_opt();
                ("".to_string(), cx, cy, window_title)
            }
        };

        prepare_hud(&window, cursor_x, cursor_y);
        if let Err(err) = window.show() {
            eprintln!("[Glint] show failed: {err}");
        }
        if let Err(err) = window.set_focus() {
            eprintln!("[Glint] set_focus failed: {err}");
        }
        eprintln!(
            "[Glint] after show: visible={:?} outer={:?}",
            window.is_visible(),
            window.outer_size()
        );
        let _ = app.emit(
            "glint-trigger",
            serde_json::json!({
                "text": text,
                "window_title": window_title
            }),
        );
    }
}

pub fn trigger_snipping_and_show(app: &AppHandle) {
    let Some(window) = app.get_webview_window("snipping") else {
        return;
    };

    #[cfg(target_os = "linux")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            match glint_core::capture_via_portal().await {
                Ok(frame) => {
                    let Some(window) = app.get_webview_window("snipping") else {
                        return;
                    };
                    // Cover the full logical desktop; the captured image keeps
                    // its real pixel size for the selection math.
                    let display = window
                        .current_monitor()
                        .ok()
                        .flatten()
                        .map(|monitor| {
                            let size = monitor.size();
                            let pos = monitor.position();
                            (pos.x, pos.y, size.width as i32, size.height as i32)
                        })
                        .unwrap_or((0, 0, 1920, 1080));

                    let _ = window.set_position(PhysicalPosition::new(display.0, display.1));
                    let _ = window.set_size(PhysicalSize::new(display.2 as u32, display.3 as u32));

                    let payload = serde_json::json!({
                        "image_data_url": frame.image_data_url,
                        "geometry": {
                            "x": display.0,
                            "y": display.1,
                            "width": frame.width,
                            "height": frame.height
                        },
                        "window_title": crate::platform::get_active_window_title_opt()
                    });
                    let _ = window.emit("glint-snipping-capture", &payload);
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                Err(err) => {
                    eprintln!("[Glint] 截屏失败: {err}");
                    let _ = app.emit("glint-screenshot-error", err.to_string());
                }
            }
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
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
fn submit_snipping_result(
    app: AppHandle,
    image_data_url: String,
    prompt: Option<String>,
    window_title: Option<String>,
) {
    if let Some(snip_win) = app.get_webview_window("snipping") {
        let _ = snip_win.hide();
    }

    if let Some(hud_win) = app.get_webview_window("hud") {
        let (cx, cy) = get_cursor_pos();
        let (w, h) = hud_outer_size(&hud_win);
        position_hud_window(&hud_win, cx, cy, None, w, h);
        let _ = hud_win.show();
        let _ = hud_win.set_focus();
        let _ = hud_win.emit(
            "glint-multimodal-trigger",
            serde_json::json!({
                "image_data_url": image_data_url,
                "prompt": prompt,
                "window_title": window_title
            }),
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 关键防御：单实例守卫 (Single Instance Guard)。
        // 杜绝重复打开进程导致任务栏托盘出现多个冗余图标，重复启动时直接激活并聚焦已有视窗。
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(hud) = app.get_webview_window("hud") {
                let (cx, cy) = get_cursor_pos();
                let (w, h) = hud_outer_size(&hud);
                position_hud_window(&hud, cx, cy, None, w, h);
                let _ = hud.show();
                let _ = hud.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 初始化窗口置顶状态
            let cfg = load_config(&app.handle());
            if let Some(hud) = app.get_webview_window("hud") {
                let _ = hud.set_always_on_top(cfg.always_on_top);
            }

            // Windows/macOS: native hotkey loop. Linux: KGlobalAccel or the
            // GlobalShortcuts portal, both driven from glint-core.
            #[cfg(not(target_os = "linux"))]
            platform::spawn_global_hotkeys(
                app.handle().clone(),
                trigger_capture_and_show,
                toggle_window_visibility,
                trigger_snipping_and_show,
            );

            #[cfg(target_os = "linux")]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let engine =
                        glint_core::PlatformHotkeyEngine::new(GLINT_APP_ID, "Glint");
                    let bindings = glint_core::default_bindings();
                    let callback: glint_core::HotkeyCallback = Arc::new(move |action| {
                        eprintln!("[Glint] hotkey action: {action:?}");
                        match action {
                            glint_core::HotkeyAction::SelectionCapture => {
                                trigger_capture_and_show(&handle)
                            }
                            glint_core::HotkeyAction::ToggleHud => {
                                toggle_window_visibility(&handle)
                            }
                            glint_core::HotkeyAction::ScreenCapture => {
                                trigger_snipping_and_show(&handle)
                            }
                        }
                    });

                    if let Err(err) = engine.register(bindings, callback).await {
                        eprintln!("[Glint] 全局热键注册失败: {err}");
                    }
                });
            }

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

            // Tauri 2 removed `Image::from_app_icon_resource`; use the bundled
            // window icon and let the platform fall back to its default if absent.
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
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
            set_hud_pinned,
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
            enable_tools: false,
            window_title: Some("System Architecture - PDF Reader".to_string()),
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

