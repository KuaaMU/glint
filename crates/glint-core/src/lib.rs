pub mod capture;
pub mod config;
pub mod hotkey;
pub mod llm;
pub mod platform;
pub mod selection;
pub mod session;
pub mod tools;

pub use capture::{capture_selected_text, get_cursor_pos, SelectionResult, ScreenCaptureResult};
pub use config::{
    default_config_path, load_config, load_default_config, save_config, save_default_config,
    AppConfig,
};
pub use hotkey::{
    default_bindings, HotkeyAction, HotkeyBinding, HotkeyCallback, HotkeyError, KeyChord,
    PlatformHotkeyEngine,
};
pub use llm::{
    build_api_payload, stream_explanation, ChatMessage, QueryRequest, StreamChunk, StreamSink,
};
pub use selection::{
    normalize_selection, platform_sensor, CaptureError, SelectionCandidate, SelectionSensor,
    SelectionSource, MAX_CHARS, MAX_RAW_BYTES, READ_TIMEOUT_MS,
};
pub use session::{detect_session, is_kde, SessionType};
pub use tools::{execute_tool, get_available_tools_schema, ToolCallPayload};
