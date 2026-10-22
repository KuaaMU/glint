pub mod capture;
pub mod config;
pub mod context;
pub mod host;
pub mod hotkey;
pub mod llm;
pub mod platform;
pub mod screen;
pub mod selection;
pub mod session;
pub mod tools;

pub use capture::{
    ScreenCaptureResult, ScreenGeometry, SelectionResult, capture_selected_text,
    encode_rgba_to_png_data_url, generate_fallback_capture, get_cursor_pos,
};
pub use config::{
    AppConfig, default_config_path, load_config, load_default_config, save_config,
    save_default_config,
};
pub use context::{
    ContextEnvelope, Continuity, Depth, TurnPlan, TurnSelection, build_messages, plan_turn,
};
pub use host::{HostIdentity, doc_key_from_title, normalize_title};
pub use hotkey::{
    HotkeyAction, HotkeyBinding, HotkeyCallback, HotkeyError, KeyChord, PlatformHotkeyEngine,
    default_bindings,
};
pub use llm::{
    ChatMessage, QueryRequest, StreamChunk, StreamSink, build_api_payload, stream_explanation,
};
pub use screen::{CapturedFrame, ScreenError, capture_via_portal, png_file_to_data_url};
pub use selection::{
    CaptureError, MAX_CHARS, MAX_RAW_BYTES, READ_TIMEOUT_MS, SelectionCandidate, SelectionSensor,
    SelectionSource, normalize_selection, platform_sensor,
};
pub use session::{SessionType, detect_session, is_kde};
pub use tools::{ToolCallPayload, execute_tool, get_available_tools_schema};
