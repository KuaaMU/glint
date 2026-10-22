//! Desktop session detection.
//!
//! This is deliberately tiny and dependency-free: the shell and the Linux
//! probes need to know "Wayland or X11" and "KDE or not" before choosing an
//! implementation.

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionType {
    Wayland,
    X11,
    Unknown,
}

impl SessionType {
    pub fn as_str(self) -> &'static str {
        match self {
            SessionType::Wayland => "wayland",
            SessionType::X11 => "x11",
            SessionType::Unknown => "unknown",
        }
    }
}

pub fn detect_session() -> SessionType {
    detect_session_from(env::var("XDG_SESSION_TYPE").ok().as_deref())
}

/// Testable variant of [`detect_session`].
pub fn detect_session_from(value: Option<&str>) -> SessionType {
    match value.map(|v| v.trim().to_ascii_lowercase()) {
        Some(v) if v == "wayland" => SessionType::Wayland,
        Some(v) if v == "x11" => SessionType::X11,
        _ => SessionType::Unknown,
    }
}

pub fn is_kde() -> bool {
    env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_ascii_lowercase().contains("kde"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wayland_and_x11() {
        assert_eq!(detect_session_from(Some("wayland")), SessionType::Wayland);
        assert_eq!(detect_session_from(Some(" X11 ")), SessionType::X11);
        assert_eq!(detect_session_from(Some("tty")), SessionType::Unknown);
        assert_eq!(detect_session_from(None), SessionType::Unknown);
    }
}
