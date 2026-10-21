//! Host identity: which application the user is reading in.
//!
//! Continuity needs a stable anchor, but window titles are both unstable
//! ("GitHub - PR #12" changes on every tab) and sensitive. `HostIdentity`
//! therefore splits what travels with the prompt from what never leaves RAM:
//!
//! * `app_class` — stable, safe to persist ("firefox", "code", "okular").
//! * `title` — RAM only, used for the HUD badge; never serialised.
//! * `doc_key` — a short digest of the normalised title, used to decide
//!   whether two triggers belong to the same reading thread.
//!
//! Nothing here sends a title to the model. The caller decides that.

/// Stable, non-sensitive identity of the frontmost application.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HostIdentity {
    /// Lowercased WM class / desktop file stem, e.g. `firefox`, `org.kde.okular`.
    pub app_class: String,
    /// Raw window title. Held in memory only; never written to disk or sent.
    pub title: Option<String>,
    /// Digest of the normalised title, for thread matching.
    pub doc_key: Option<String>,
}

impl HostIdentity {
    pub fn new(app_class: impl Into<String>, title: Option<String>) -> Self {
        let app_class = app_class.into().trim().to_lowercase();
        let doc_key = title.as_deref().and_then(doc_key_from_title);
        Self {
            app_class,
            title: title.filter(|t| !t.trim().is_empty()),
            doc_key,
        }
    }

    /// The label shown in the HUD. Falls back to the class name so the badge
    /// never goes blank when a window has no title.
    pub fn display_label(&self) -> Option<&str> {
        self.title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .or_else(|| Some(self.app_class.as_str()).filter(|c| !c.is_empty()))
    }

    /// Whether a new trigger belongs to the same reading thread.
    ///
    /// Requires the same app. When both sides have a `doc_key` it must match;
    /// generic titles ("New Tab", empty) produce no key and fall back to
    /// same-app matching, which the caller further bounds with an idle timer.
    pub fn same_thread_as(&self, other: &HostIdentity) -> bool {
        if self.app_class.is_empty() || self.app_class != other.app_class {
            return false;
        }
        match (&self.doc_key, &other.doc_key) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
}

/// Titles that identify no document. Treated as "unknown document" so two
/// unrelated tabs are not silently merged into one thread.
const GENERIC_TITLES: &[&str] = &[
    "new tab",
    "new window",
    "untitled",
    "无标题",
    "新标签页",
    "新窗口",
    "about:blank",
    "about:newtab",
    "start page",
    "home",
];

/// Normalise a title for matching: drop volatile suffixes, collapse
/// whitespace, lowercase.
///
/// Browser titles are usually `<document> - <browser>`, and editors append
/// modification markers. Keeping the document part is what makes two triggers
/// in the same tab match.
pub fn normalize_title(title: &str) -> String {
    let mut text = title.trim().to_lowercase();

    // " — Mozilla Firefox", " - Google Chrome", " — Konsole"
    for suffix in [
        " - mozilla firefox",
        " — mozilla firefox",
        " - google chrome",
        " — google chrome",
        " - chromium",
        " — chromium",
        " - microsoft edge",
        " - vivaldi",
        " - brave",
    ] {
        if let Some(stripped) = text.strip_suffix(suffix) {
            text = stripped.to_string();
            break;
        }
    }

    // Leading modification markers from editors: "● file.rs", "* file.txt"
    text = text
        .trim_start_matches(['●', '*', '•'])
        .trim()
        .to_string();

    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A stable 16-hex-char digest of the normalised title, or `None` when the
/// title identifies no document.
pub fn doc_key_from_title(title: &str) -> Option<String> {
    let normalized = normalize_title(title);
    if normalized.is_empty() || GENERIC_TITLES.contains(&normalized.as_str()) {
        return None;
    }
    // FNV-1a: no dependency, deterministic across runs. This is a matching
    // key, not a security boundary.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normalized.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    Some(format!("{hash:016x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_app_and_title_is_the_same_thread() {
        let a = HostIdentity::new("firefox", Some("Rust async book - Mozilla Firefox".into()));
        let b = HostIdentity::new("Firefox", Some("Rust async book - Mozilla Firefox".into()));
        assert!(a.same_thread_as(&b));
        assert_eq!(a.doc_key, b.doc_key);
    }

    #[test]
    fn different_documents_in_one_browser_do_not_merge() {
        let a = HostIdentity::new("firefox", Some("Rust async book - Mozilla Firefox".into()));
        let b = HostIdentity::new("firefox", Some("Tokio tutorial - Mozilla Firefox".into()));
        assert!(!a.same_thread_as(&b));
    }

    #[test]
    fn different_apps_never_share_a_thread() {
        let a = HostIdentity::new("code", Some("main.rs - glint".into()));
        let b = HostIdentity::new("firefox", Some("main.rs - glint".into()));
        assert!(!a.same_thread_as(&b));
    }

    #[test]
    fn generic_titles_fall_back_to_same_app() {
        let a = HostIdentity::new("firefox", Some("New Tab".into()));
        assert!(a.doc_key.is_none());
        let b = HostIdentity::new("firefox", Some("New Tab".into()));
        assert!(a.same_thread_as(&b));
    }

    #[test]
    fn missing_title_still_matches_within_the_app() {
        let a = HostIdentity::new("firefox", None);
        let b = HostIdentity::new("firefox", Some("Tokio tutorial - Mozilla Firefox".into()));
        assert!(a.same_thread_as(&b));
    }

    #[test]
    fn strips_browser_suffix_and_editor_marker() {
        assert_eq!(
            normalize_title("● main.rs - Visual Studio Code"),
            "main.rs - visual studio code"
        );
        assert_eq!(
            normalize_title("Async in depth - Tokio  - Mozilla Firefox"),
            "async in depth - tokio"
        );
    }

    #[test]
    fn display_label_prefers_title_then_class() {
        let with_title = HostIdentity::new("firefox", Some(" Tokio ".into()));
        assert_eq!(with_title.display_label(), Some("Tokio"));
        let no_title = HostIdentity::new("firefox", None);
        assert_eq!(no_title.display_label(), Some("firefox"));
    }

    #[test]
    fn doc_key_is_stable_across_runs() {
        assert_eq!(
            doc_key_from_title("Rust async book"),
            doc_key_from_title("  RUST   async book  ")
        );
    }
}
