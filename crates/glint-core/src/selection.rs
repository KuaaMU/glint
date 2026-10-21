//! Selection capture.
//!
//! The contract is intentionally narrow: a sensor returns one bounded
//! [`SelectionCandidate`] or a typed [`CaptureError`]. It never writes to the
//! user's clipboard, never synthesises Ctrl+C, and never falls back to a stale
//! candidate.

use std::time::Instant;

use crate::session::{SessionType, detect_session};

/// Hard cap on the bytes read from the selection owner.
pub const MAX_RAW_BYTES: usize = 64 * 1024;
/// Hard cap on the characters forwarded to the model.
pub const MAX_CHARS: usize = 3000;
/// Maximum wait for the selection owner to produce data.
pub const READ_TIMEOUT_MS: i32 = 150;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionSource {
    WaylandPrimary,
    X11Primary,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SelectionCandidate {
    pub text: String,
    pub original_len: usize,
    pub truncated: bool,
    pub source: SelectionSource,
    pub captured_at: Instant,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no selection available")]
    NoSelection,
    #[error("selection read timed out")]
    Timeout,
    #[error("selection backend unavailable: {0}")]
    Unavailable(String),
    #[error("selection read failed: {0}")]
    Io(String),
}

pub trait SelectionSensor: Send + Sync {
    fn capture(&self) -> Result<SelectionCandidate, CaptureError>;
}

/// Trim NULs/whitespace and apply the Unicode-safe 3000-character cap.
pub fn normalize_selection(raw: &str) -> (String, usize, bool) {
    let trimmed = raw.trim().trim_matches('\0').trim();
    let original_len = trimmed.chars().count();
    if original_len > MAX_CHARS {
        (
            trimmed.chars().take(MAX_CHARS).collect(),
            original_len,
            true,
        )
    } else {
        (trimmed.to_string(), original_len, false)
    }
}

#[cfg(target_os = "linux")]
pub struct LinuxSelectionSensor;

#[cfg(target_os = "linux")]
impl SelectionSensor for LinuxSelectionSensor {
    fn capture(&self) -> Result<SelectionCandidate, CaptureError> {
        let (raw, source) = match detect_session() {
            SessionType::Wayland => match wayland_primary() {
                Ok(text) => (text, SelectionSource::WaylandPrimary),
                // Only protocol unavailability earns an XWayland fallback.
                // An empty or timed-out PRIMARY is a real "no selection",
                // and falling back could resurrect a stale X11 selection.
                Err(CaptureError::Unavailable(_)) => {
                    let text = x11_primary()?;
                    (text, SelectionSource::X11Primary)
                }
                Err(err) => return Err(err),
            },
            SessionType::X11 => (x11_primary()?, SelectionSource::X11Primary),
            SessionType::Unknown => match wayland_primary() {
                Ok(text) => (text, SelectionSource::WaylandPrimary),
                Err(CaptureError::Unavailable(_)) => (x11_primary()?, SelectionSource::X11Primary),
                Err(err) => return Err(err),
            },
        };

        let (text, original_len, truncated) = normalize_selection(&raw);
        if text.is_empty() {
            return Err(CaptureError::NoSelection);
        }

        Ok(SelectionCandidate {
            text,
            original_len,
            truncated,
            source,
            captured_at: Instant::now(),
        })
    }
}

#[cfg(target_os = "linux")]
fn wayland_primary() -> Result<String, CaptureError> {
    use std::io::Read;
    use std::os::fd::AsRawFd;
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};

    let (mut pipe, _mime) = get_contents(ClipboardType::Primary, Seat::Unspecified, MimeType::Text)
        .map_err(map_wl_error)?;

    let mut poll_fd = libc::pollfd {
        fd: pipe.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    // SAFETY: poll_fd is a valid, initialized pollfd for the duration of the call.
    let ready = unsafe { libc::poll(&mut poll_fd, 1, READ_TIMEOUT_MS) };
    if ready == 0 {
        return Err(CaptureError::Timeout);
    }
    if ready < 0 {
        return Err(CaptureError::Io(
            std::io::Error::last_os_error().to_string(),
        ));
    }

    let mut buffer = Vec::with_capacity(4096);
    pipe.by_ref()
        .take(MAX_RAW_BYTES as u64)
        .read_to_end(&mut buffer)
        .map_err(|err| CaptureError::Io(err.to_string()))?;

    String::from_utf8(buffer)
        .map_err(|err| CaptureError::Io(format!("selection is not valid UTF-8: {err}")))
}

#[cfg(target_os = "linux")]
fn map_wl_error(err: wl_clipboard_rs::paste::Error) -> CaptureError {
    use wl_clipboard_rs::paste::Error;

    match err {
        Error::NoSeats | Error::ClipboardEmpty | Error::NoMimeType => CaptureError::NoSelection,
        other => CaptureError::Unavailable(other.to_string()),
    }
}

#[cfg(target_os = "linux")]
fn x11_primary() -> Result<String, CaptureError> {
    let clipboard = x11_clipboard::Clipboard::new()
        .map_err(|err| CaptureError::Unavailable(err.to_string()))?;
    let atoms = &clipboard.getter.atoms;

    let data = clipboard
        .load(
            atoms.primary,
            atoms.utf8_string,
            atoms.property,
            std::time::Duration::from_millis(READ_TIMEOUT_MS as u64),
        )
        .map_err(|err| CaptureError::Io(err.to_string()))?;

    String::from_utf8(data)
        .map_err(|err| CaptureError::Io(format!("selection is not valid UTF-8: {err}")))
}

#[cfg(not(target_os = "linux"))]
pub struct UnsupportedSelectionSensor;

#[cfg(not(target_os = "linux"))]
impl SelectionSensor for UnsupportedSelectionSensor {
    fn capture(&self) -> Result<SelectionCandidate, CaptureError> {
        Err(CaptureError::Unavailable(
            "no selection backend for this platform".to_string(),
        ))
    }
}

/// The default sensor for the current platform.
pub fn platform_sensor() -> Box<dyn SelectionSensor> {
    #[cfg(target_os = "linux")]
    {
        Box::new(LinuxSelectionSensor)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(UnsupportedSelectionSensor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_nulls_and_whitespace() {
        let (text, len, truncated) = normalize_selection("\0  hello  \0");
        assert_eq!(text, "hello");
        assert_eq!(len, 5);
        assert!(!truncated);
    }

    #[test]
    fn truncates_at_unicode_character_boundary() {
        let input = "a".repeat(MAX_CHARS + 1);
        let (text, len, truncated) = normalize_selection(&input);
        assert_eq!(text.chars().count(), MAX_CHARS);
        assert_eq!(len, MAX_CHARS + 1);
        assert!(truncated);
    }

    #[test]
    fn preserves_astral_plane_characters() {
        let input = "👨‍👩‍👦🏳️‍🌈".repeat(1000);
        let (text, len, truncated) = normalize_selection(&input);
        assert!(truncated);
        assert_eq!(len, input.chars().count());
        assert_eq!(text.chars().count(), MAX_CHARS);
    }

    #[test]
    fn empty_selection_is_a_specific_error() {
        let (text, _, _) = normalize_selection("   \0 ");
        assert!(text.is_empty());
    }

    #[test]
    #[ignore = "requires a live Wayland/X11 session with a PRIMARY selection"]
    fn live_primary_smoke() {
        let candidate = platform_sensor().capture().expect("live PRIMARY capture");
        assert!(!candidate.text.is_empty());
        assert!(candidate.text.chars().count() <= MAX_CHARS);
    }
}
