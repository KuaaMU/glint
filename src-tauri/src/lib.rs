mod capture;
mod config;
mod llm;
mod placement;
mod platform;
mod preferences;
pub mod tools;

use capture::{SelectionResult, capture_selected_text, get_cursor_pos};
use config::load_config;
use llm::{QueryRequest, StreamChunk, stream_explanation};
use preferences::{
    fetch_models, get_app_config, hide_preferences_window, open_preferences_window,
    test_llm_connection, update_app_config,
};
#[cfg(target_os = "linux")]
use std::sync::Arc;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow,
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

#[cfg(target_os = "linux")]
const GLINT_APP_ID: &str = "com.glint.desktop";

/// Default companion card size (logical pixels). Tall enough for ~12 lines of
/// answer plus the follow-up bar, while staying compact next to a selection.
const CARD_W: f64 = 440.0;
const CARD_H: f64 = 560.0;

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

/// All monitors as plain rects. Geometry decisions stay in `placement`, so no
/// X11 coordinate maths leaks into window or business code.
fn monitor_rects(window: &WebviewWindow) -> Vec<placement::Rect> {
    window
        .available_monitors()
        .unwrap_or_default()
        .iter()
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
        .collect()
}

fn monitor_rect_at(window: &WebviewWindow, x: i32, y: i32) -> Option<placement::Rect> {
    placement::monitor_at_point(&monitor_rects(window), placement::Point { x, y })
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
    // Use the monitor under the cursor, not the one the HUD last lived on.
    // `current_monitor()` is stale while the window is hidden elsewhere, and
    // clamping into it pushed the card onto the wrong display.
    let monitor_rect = monitor_rect_at(window, cursor_x, cursor_y)
        .or_else(|| {
            window.current_monitor().ok().flatten().map(|monitor| {
                let size = monitor.size();
                let pos = monitor.position();
                placement::Rect {
                    x: pos.x,
                    y: pos.y,
                    width: size.width as i32,
                    height: size.height as i32,
                }
            })
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
    //
    // The position is only valid after the surface is mapped, so read it back
    // on a short delayed task rather than immediately: a fresh WebKitGTK
    // window reports (0, 0) until its first frame is committed.
    let window = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if !window.is_visible().unwrap_or(false) {
            return;
        }
        let Ok(actual) = window.outer_position() else {
            return;
        };
        let dx = (actual.x - target.x).abs();
        let dy = (actual.y - target.y).abs();
        if dx > 8 || dy > 8 {
            eprintln!(
                "[Glint] placement not honoured: wanted ({}, {}), got ({}, {}); \
                 run with GDK_BACKEND=x11 for cursor-adjacent placement",
                target.x, target.y, actual.x, actual.y
            );
        } else {
            eprintln!("[Glint] placement honoured at ({}, {})", actual.x, actual.y);
        }
    });
}

/// Current outer size in physical pixels, defaulting to the configured card.
fn hud_outer_size(window: &WebviewWindow) -> (i32, i32) {
    match window.outer_size() {
        Ok(size) if size.width > 0 && size.height > 0 => (size.width as i32, size.height as i32),
        _ => {
            let scale = window.scale_factor().unwrap_or(1.0);
            (
                (CARD_W * scale).round() as i32,
                (CARD_H * scale).round() as i32,
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
        let _ = window.set_size(LogicalSize::new(CARD_W, CARD_H));
    }
}

/// Whether the user pinned the card. A pinned HUD stays visible across new
/// selections; Escape and the close button still dismiss it.
static HUD_PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the HUD was on screen when the snip overlay was opened. The HUD is
/// hidden before the portal grabs the desktop (otherwise it lands in the
/// screenshot) and restored on cancel, not on submit.
static HUD_VISIBLE_BEFORE_SNIP: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

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
        if let Err(err) = window.set_size(LogicalSize::new(CARD_W, CARD_H)) {
            eprintln!("[Glint] set_size failed: {err}");
        }
        // A window created hidden keeps a 0x0 surface until it is shown; the
        // size reported right here is not the size it will get. Re-apply the
        // size after mapping so the card never opens as a thin strip.
        let handle = window.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(80)).await;
            if handle.is_visible().unwrap_or(false) {
                let _ = handle.set_size(LogicalSize::new(CARD_W, CARD_H));
                eprintln!("[Glint] post-show size {:?}", handle.outer_size());
            }
        });
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

        // MUST be read before `show()`: once the HUD takes focus, KWin reports
        // "Glint" as the foreground app and every selection would look like the
        // same document, silently merging unrelated threads.
        let host = crate::platform::get_active_window_identity();
        let window_title = window_title.or_else(|| host.title.clone());
        eprintln!(
            "[Glint] host: class={:?} doc_key={:?}",
            host.app_class, host.doc_key
        );

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
                "window_title": window_title,
                "app_class": host.app_class,
                "doc_key": host.doc_key
            }),
        );
    }
}

/// Bounding box of every monitor, in physical desktop coordinates.
///
/// The Screenshot portal returns one image of the whole virtual desktop, so the
/// overlay must cover that same rectangle. Sizing it to `current_monitor()`
/// squeezed all displays into one monitor's frame.
#[cfg(target_os = "linux")]
fn virtual_desktop_bounds(window: &WebviewWindow) -> (i32, i32, u32, u32) {
    let mut rects = monitor_rects(window);
    if rects.is_empty()
        && let Some(monitor) = window.current_monitor().ok().flatten()
    {
        let size = monitor.size();
        let pos = monitor.position();
        rects.push(placement::Rect {
            x: pos.x,
            y: pos.y,
            width: size.width as i32,
            height: size.height as i32,
        });
    }

    let bounds = placement::bounding_box(&rects);
    (
        bounds.x,
        bounds.y,
        bounds.width as u32,
        bounds.height as u32,
    )
}

/// The monitor the user is working on: the one containing the pointer.
#[cfg(target_os = "linux")]
fn active_monitor_rect(app: &AppHandle) -> Option<(placement::Rect, f64)> {
    let window = app.get_webview_window("hud")?;
    let (cx, cy) = get_cursor_pos();
    let rects = monitor_rects(&window);
    if rects.is_empty() {
        return None;
    }

    let rect = placement::monitor_at_point(&rects, placement::Point { x: cx, y: cy })
        .or_else(|| rects.first().copied())?;

    // Pick the scale factor of the matching Tauri monitor; geometry alone does
    // not identify which display a rect came from.
    let scale = window
        .available_monitors()
        .unwrap_or_default()
        .iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            pos.x == rect.x && pos.y == rect.y && size.width as i32 == rect.width
        })
        .map(|m| m.scale_factor())
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));

    Some((rect, scale))
}

pub fn trigger_snipping_and_show(app: &AppHandle) {
    if app.get_webview_window("snipping").is_none() {
        return;
    }

    // Keep the HUD out of the capture. Compositors need a beat to actually
    // unmap the window, so the Linux path waits before asking the portal.
    if let Some(hud) = app.get_webview_window("hud") {
        let visible = hud.is_visible().unwrap_or(false);
        HUD_VISIBLE_BEFORE_SNIP.store(visible, std::sync::atomic::Ordering::SeqCst);
        if visible {
            let _ = hud.hide();
        }
    }

    #[cfg(target_os = "linux")]
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            // Let the hidden HUD disappear from the compositor's next frame.
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;

            match glint_core::capture_via_portal().await {
                Ok(frame) => {
                    let Some(window) = app.get_webview_window("snipping") else {
                        return;
                    };

                    // The portal captures the whole virtual desktop, but KWin
                    // confines a toplevel overlay to one monitor's work area
                    // (measured: a 2987x3280 request came back 2987x1610). So
                    // cover only the monitor under the pointer and crop that
                    // monitor's slice out of the capture instead of squeezing
                    // every display into one.
                    let (rect, _scale) = active_monitor_rect(&app).unwrap_or((
                        placement::Rect {
                            x: 0,
                            y: 0,
                            width: frame.width as i32,
                            height: frame.height as i32,
                        },
                        1.0,
                    ));
                    // The image's (0,0) is the virtual-desktop bounding-box
                    // origin, which is negative when a monitor sits left of or
                    // above the primary one.
                    let (bx, by, _, _) = virtual_desktop_bounds(&window);

                    // Identity of the app the user snipped from. Read before the
                    // overlay maps so it cannot describe the overlay itself.
                    let host = crate::platform::get_active_window_identity();

                    let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
                    let _ =
                        window.set_size(PhysicalSize::new(rect.width as u32, rect.height as u32));
                    eprintln!(
                        "[Glint] snip overlay=({},{}) {}x{}, capture={}x{}",
                        rect.x, rect.y, rect.width, rect.height, frame.width, frame.height
                    );

                    let payload = serde_json::json!({
                        "image_data_url": frame.image_data_url,
                        // Device-pixel geometry of the captured virtual desktop.
                        "capture": {
                            "width": frame.width,
                            "height": frame.height,
                            "origin_x": bx,
                            "origin_y": by
                        },
                        // The monitor the overlay sits on, in desktop coords.
                        // The frontend intersects this with the window's real
                        // (work-area-clipped) size to crop 1:1, without ever
                        // scaling the image.
                        "monitor": {
                            "x": rect.x,
                            "y": rect.y,
                            "width": rect.width,
                            "height": rect.height
                        },
                        "window_title": crate::platform::get_active_window_title_opt(),
                        "app_class": host.app_class,
                        "doc_key": host.doc_key
                    });
                    let _ = window.emit("glint-snipping-capture", &payload);
                    let _ = window.show();
                    let _ = window.set_focus();

                    // The compositor decides the final geometry (KWin keeps the
                    // top panel strip free), and the frontend cannot infer it
                    // from `window.screenX` under fractional scaling. Report the
                    // mapped physical rect so the crop is exact.
                    let verify = window.clone();
                    tauri::async_runtime::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        let pos = verify.outer_position();
                        let size = verify.outer_size();
                        eprintln!("[Glint] snip window after show: pos={pos:?} size={size:?}");
                        if let (Ok(pos), Ok(size)) = (pos, size) {
                            let _ = verify.emit(
                                "glint-snipping-geometry",
                                serde_json::json!({
                                    "x": pos.x,
                                    "y": pos.y,
                                    "width": size.width,
                                    "height": size.height
                                }),
                            );
                        }
                    });
                }
                Err(err) => {
                    eprintln!("[Glint] 截屏失败: {err}");
                    let _ = app.emit("glint-screenshot-error", err.to_string());
                    if HUD_VISIBLE_BEFORE_SNIP.swap(false, std::sync::atomic::Ordering::SeqCst)
                        && let Some(hud) = app.get_webview_window("hud")
                    {
                        let _ = hud.show();
                    }
                }
            }
        });
    }

    #[cfg(not(target_os = "linux"))]
    {
        let Some(window) = app.get_webview_window("snipping") else {
            return;
        };
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
                if HUD_VISIBLE_BEFORE_SNIP.swap(false, std::sync::atomic::Ordering::SeqCst) {
                    if let Some(hud) = app.get_webview_window("hud") {
                        let _ = hud.show();
                    }
                }
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
    // Cancel: put the companion card back exactly where it was.
    if HUD_VISIBLE_BEFORE_SNIP.swap(false, std::sync::atomic::Ordering::SeqCst)
        && let Some(hud) = app.get_webview_window("hud")
    {
        let _ = hud.show();
    }
}

#[tauri::command]
fn submit_snipping_result(
    app: AppHandle,
    image_data_url: String,
    prompt: Option<String>,
    window_title: Option<String>,
    app_class: Option<String>,
    doc_key: Option<String>,
) {
    if let Some(snip_win) = app.get_webview_window("snipping") {
        let _ = snip_win.hide();
    }
    // Submit always brings the card back with the new turn; the cancel flag no
    // longer applies.
    HUD_VISIBLE_BEFORE_SNIP.store(false, std::sync::atomic::Ordering::SeqCst);

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
                "window_title": window_title,
                "app_class": app_class,
                "doc_key": doc_key
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
            let cfg = load_config(app.handle());
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
                    let engine = glint_core::PlatformHotkeyEngine::new(GLINT_APP_ID, "Glint");
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
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
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
            if (window.label() == "preferences" || window.label() == "snipping")
                && let tauri::WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
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

        assert!(
            payload["image_data_url"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert_eq!(payload["prompt"].as_str().unwrap(), "请分析此屏幕截图区域");
    }

    #[test]
    fn test_multimodal_multi_turn_payload_generation() {
        let req = llm::QueryRequest {
            messages: vec![
                llm::ChatMessage::user_multimodal("请分析此截屏重点", "data:image/png;base64,AAAA"),
                llm::ChatMessage::assistant(
                    "这是包含系统架构图的截屏，标注重点在于高速缓存和内存拓扑。",
                ),
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
