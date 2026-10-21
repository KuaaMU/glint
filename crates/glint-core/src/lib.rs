pub mod capture;
pub mod config;
pub mod llm;
pub mod platform;
pub mod tools;

pub use capture::{capture_selected_text, get_cursor_pos, SelectionResult, ScreenCaptureResult};
pub use config::{
    default_config_path, load_config, load_default_config, save_config, save_default_config,
    AppConfig,
};
pub use llm::{
    build_api_payload, stream_explanation, ChatMessage, QueryRequest, StreamChunk, StreamSink,
};
pub use tools::{execute_tool, get_available_tools_schema, ToolCallPayload};
