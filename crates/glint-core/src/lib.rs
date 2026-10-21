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
    capture_selected_text, encode_rgba_to_png_data_url, generate_fallback_capture, get_cursor_pos,
    ScreenCaptureResult, ScreenGeometry, SelectionResult,
};
pub use config::{
    default_config_path, load_config, load_default_config, save_config, save_default_config,
    AppConfig,
};
pub use hotkey::{
    default_bindings, HotkeyAction, HotkeyBinding, HotkeyCallback, HotkeyError, KeyChord,
    PlatformHotkeyEngine,
};
pub use context::{
    build_messages, plan_turn, Continuity, ContextEnvelope, Depth, TurnPlan, TurnSelection,
};
pub use host::{doc_key_from_title, normalize_title, HostIdentity};
pub use llm::{
    build_api_payload, stream_explanation, ChatMessage, QueryRequest, StreamChunk, StreamSink,
};
pub use screen::{capture_via_portal, png_file_to_data_url, CapturedFrame, ScreenError};
pub use selection::{
    normalize_selection, platform_sensor, CaptureError, SelectionCandidate, SelectionSensor,
    SelectionSource, MAX_CHARS, MAX_RAW_BYTES, READ_TIMEOUT_MS,
};
pub use session::{detect_session, is_kde, SessionType};
pub use tools::{execute_tool, get_available_tools_schema, ToolCallPayload};
