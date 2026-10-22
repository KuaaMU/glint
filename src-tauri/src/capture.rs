//! Tauri-side adapters for platform capture.
//!
//! Text selection on Windows keeps the Win32 clipboard-sequence engine; Linux
//! uses the PRIMARY-selection sensor from `glint-core` (no Ctrl+C synthesis, no
//! clipboard writes). Shared types and PNG encoding live in `glint-core`.

use crate::platform::get_cursor_position;

#[cfg(not(target_os = "linux"))]
use crate::platform::simulate_copy;
#[cfg(not(target_os = "linux"))]
use std::thread::sleep;
#[cfg(not(target_os = "linux"))]
use std::time::Duration;

#[cfg(test)]
pub use glint_core::encode_rgba_to_png_data_url;
pub use glint_core::{ScreenCaptureResult, SelectionResult};
// Only the placeholder screenshot above uses this.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub use glint_core::generate_fallback_capture;

#[cfg(not(target_os = "linux"))]
use arboard::Clipboard;

#[cfg(target_os = "windows")]
pub fn capture_screen() -> Result<ScreenCaptureResult, String> {
    glint_core::capture::capture_screen()
}

#[cfg(target_os = "macos")]
pub fn capture_screen() -> Result<ScreenCaptureResult, String> {
    glint_core::capture::capture_screen()
}

// Linux never reaches this: `trigger_snipping_and_show` uses the XDG portal.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn capture_screen() -> Result<ScreenCaptureResult, String> {
    Ok(generate_fallback_capture(800, 600, 0, 0))
}

pub fn get_cursor_pos() -> (i32, i32) {
    get_cursor_position()
}

#[cfg(target_os = "linux")]
pub fn capture_selected_text() -> Option<SelectionResult> {
    use glint_core::platform_sensor;

    let candidate = platform_sensor().capture().ok()?;
    let (cursor_x, cursor_y) = get_cursor_position();
    Some(SelectionResult {
        text: candidate.text,
        cursor_x,
        cursor_y,
        window_title: crate::platform::get_active_window_title_opt(),
    })
}

#[cfg(not(target_os = "linux"))]
fn get_clipboard_with_retry() -> Option<Clipboard> {
    for _ in 0..5 {
        if let Ok(c) = Clipboard::new() {
            return Some(c);
        }
        sleep(Duration::from_millis(6));
    }
    None
}

/// Current clipboard text, trimmed, or `None` when empty/unavailable.
#[cfg(not(target_os = "linux"))]
fn clipboard_text() -> Option<String> {
    let mut clip = get_clipboard_with_retry()?;
    let text = clip.get_text().ok()?;
    let trimmed = text.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Windows/macOS selection capture: synthesise copy and wait for the clipboard
/// to change, then restore nothing (the caller owns nothing).
#[cfg(not(target_os = "linux"))]
pub fn capture_selected_text() -> Option<SelectionResult> {
    let (cursor_x, cursor_y) = get_cursor_pos();

    let initial_seq = crate::platform::get_clipboard_sequence();
    let original_snapshot = clipboard_text();

    simulate_copy();

    let mut captured_text: Option<String> = None;
    let start = std::time::Instant::now();

    while start.elapsed().as_millis() < 350 {
        sleep(Duration::from_millis(15));

        let current_seq = crate::platform::get_clipboard_sequence();
        let seq_changed = initial_seq != 0 && current_seq != initial_seq;

        // A changed sequence number means the app answered our copy. When the
        // platform reports no sequence at all, fall back to comparing text.
        let candidate = if seq_changed {
            sleep(Duration::from_millis(10));
            clipboard_text()
        } else if initial_seq == 0 {
            clipboard_text().filter(|text| Some(text) != original_snapshot.as_ref())
        } else {
            None
        };

        if let Some(text) = candidate {
            captured_text = Some(text);
            break;
        }
    }

    if captured_text.is_none()
        && let Some(text) = clipboard_text()
        && Some(&text) != original_snapshot.as_ref()
    {
        captured_text = Some(text);
    }

    let window_title = crate::platform::get_active_window_title_opt();

    captured_text.map(|text| SelectionResult {
        text,
        cursor_x,
        cursor_y,
        window_title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn test_selection_result_serialization() {
        let res = SelectionResult {
            text: "Hello Glint".to_string(),
            cursor_x: 100,
            cursor_y: 200,
            window_title: Some("Bilibili - Google Chrome".to_string()),
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("Hello Glint"));
        assert!(json.contains("\"cursor_x\":100"));
        assert!(json.contains("Bilibili - Google Chrome"));
    }

    #[test]
    fn test_encode_rgba_to_png_data_url_valid() {
        let rgba = vec![
            255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
        ];
        let data_url =
            encode_rgba_to_png_data_url(&rgba, 2, 2).expect("encoding PNG should succeed");
        assert!(data_url.starts_with("data:image/png;base64,"));
        let b64_part = data_url.trim_start_matches("data:image/png;base64,");
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64_part)
            .expect("base64 decode should succeed");
        assert_eq!(&decoded[0..4], &[0x89, 0x50, 0x4E, 0x47]);
    }

    #[test]
    fn test_encode_rgba_to_png_data_url_invalid_length() {
        let rgba = vec![255, 0, 0];
        assert!(encode_rgba_to_png_data_url(&rgba, 2, 2).is_err());
    }

    #[test]
    fn test_screen_capture_smoke() {
        let cap = capture_screen().expect("capture_screen should succeed");
        assert!(cap.geometry.width > 0);
        assert!(cap.geometry.height > 0);
        assert!(cap.image_data_url.starts_with("data:image/png;base64,"));
    }
}
