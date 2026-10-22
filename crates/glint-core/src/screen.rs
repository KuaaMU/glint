//! Screen capture.
//!
//! On Linux the only supported path is the
//! `org.freedesktop.portal.Screenshot` portal: KWin exposes no
//! `wlr-screencopy`, and reading the framebuffer directly is not allowed on
//! Wayland. The portal returns a file URI, which is read back and encoded as a
//! PNG data URL for the existing snip UI.

use std::path::Path;

/// A captured frame plus the geometry the snip overlay should cover.
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub image_data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ScreenError {
    #[error("screen capture portal unavailable: {0}")]
    Unavailable(String),
    #[error("screen capture was cancelled or denied")]
    Denied,
    #[error("screen capture failed: {0}")]
    Failed(String),
}

/// Read a PNG file and return it as a `data:` URL.
pub fn png_file_to_data_url(path: &Path) -> Result<CapturedFrame, ScreenError> {
    let bytes = std::fs::read(path).map_err(|err| ScreenError::Failed(err.to_string()))?;
    let (width, height) = png_dimensions(&bytes)
        .ok_or_else(|| ScreenError::Failed("captured file is not a PNG".to_string()))?;

    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(CapturedFrame {
        image_data_url: format!("data:image/png;base64,{encoded}"),
        width,
        height,
    })
}

/// Minimal PNG header parser (IHDR is always the first chunk).
fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 24 || bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((width, height))
}

/// Decode `%XX` escapes in a `file://` URI path. Paths with spaces are common
/// for portal screenshots, so this cannot be skipped.
#[cfg(any(target_os = "linux", test))]
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "linux")]
pub async fn capture_via_portal() -> Result<CapturedFrame, ScreenError> {
    use ashpd::desktop::screenshot::Screenshot;

    // `interactive(false)` = grab the whole screen silently; `interactive(true)`
    // would show the compositor's region picker.
    let request = Screenshot::request()
        .interactive(false)
        .modal(false)
        .send()
        .await
        .map_err(|err| ScreenError::Unavailable(err.to_string()))?;

    let response = request.response().map_err(|err| {
        let text = err.to_string();
        if text.to_ascii_lowercase().contains("cancel")
            || text.to_ascii_lowercase().contains("denied")
        {
            ScreenError::Denied
        } else {
            ScreenError::Failed(text)
        }
    })?;

    // The portal returns a file:// URI; ashpd's Uri only exposes the string.
    let uri = response.uri().as_str().to_string();
    let encoded_path = uri
        .strip_prefix("file://")
        .ok_or_else(|| ScreenError::Failed(format!("portal returned a non-file URI: {uri}")))?;
    let path = percent_decode(encoded_path);

    let frame = png_file_to_data_url(Path::new(&path))?;
    // The portal writes a temp file; do not leave it behind.
    let _ = std::fs::remove_file(&path);
    Ok(frame)
}

#[cfg(not(target_os = "linux"))]
pub async fn capture_via_portal() -> Result<CapturedFrame, ScreenError> {
    Err(ScreenError::Unavailable(
        "portal capture is Linux-only".to_string(),
    ))
}

/// Capture the main display with Apple's `screencapture`.
///
/// `screencapture -x` is the only path that works without linking
/// ScreenCaptureKit or asking for a custom permission prompt. It needs the
/// Screen Recording permission; without it macOS either fails or returns a
/// desktop-only image, and we cannot tell those apart from the exit code.
///
/// Multi-display Macs capture the main display only. The overlay and the crop
/// use the image's own size, so the result is correct, just limited to one
/// screen.
#[cfg(target_os = "macos")]
pub fn capture_via_screencapture() -> Result<CapturedFrame, ScreenError> {
    use std::process::Command;

    let path = std::env::temp_dir().join(format!(
        "glint-screen-{}-{}.png",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    let output = Command::new("/usr/sbin/screencapture")
        .args(["-x", "-t", "png"])
        .arg(&path)
        .output()
        .map_err(|err| ScreenError::Unavailable(err.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let _ = std::fs::remove_file(&path);
        return Err(ScreenError::Failed(if stderr.is_empty() {
            "screencapture exited with an error; check the Screen Recording permission".to_string()
        } else {
            stderr
        }));
    }

    let frame = png_file_to_data_url(&path);
    // The temp file is ours; never leave it behind.
    let _ = std::fs::remove_file(&path);
    frame
}

#[cfg(not(target_os = "macos"))]
pub fn capture_via_screencapture() -> Result<CapturedFrame, ScreenError> {
    Err(ScreenError::Unavailable(
        "screencapture is macOS-only".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_png_dimensions() {
        // 1x1 transparent PNG.
        let png: [u8; 67] = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        assert_eq!(png_dimensions(&png), Some((1, 1)));
    }

    #[test]
    fn rejects_non_png() {
        assert_eq!(png_dimensions(b"not a png file at all...."), None);
    }

    #[test]
    fn decodes_portal_file_uris() {
        assert_eq!(percent_decode("/tmp/a%20b.png"), "/tmp/a b.png");
        assert_eq!(percent_decode("/tmp/plain.png"), "/tmp/plain.png");
        assert_eq!(percent_decode("/tmp/%E4%B8%AD.png"), "/tmp/中.png");
    }
}
