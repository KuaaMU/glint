pub mod capture;
pub mod config;
pub mod llm;
pub mod platform;
pub mod tools;

pub use capture::{capture_selected_text, get_cursor_pos, SelectionResult, ScreenCaptureResult};
pub use config::{load_config, save_config, AppConfig};
pub use llm::{stream_explanation, build_api_payload, ChatMessage, QueryRequest, StreamChunk};
pub use tools::{execute_tool, get_available_tools_schema, ToolCallPayload};
