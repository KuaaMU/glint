//! Tauri adapter for the headless cognitive engine.
//!
//! All prompt construction and SSE handling live in `glint-core`; this module
//! only bridges that engine's `StreamSink` to a Tauri IPC channel.

use std::sync::Arc;

use tauri::ipc::Channel;

#[cfg(test)]
pub use glint_core::{ChatMessage, build_api_payload};
pub use glint_core::{
    QueryRequest, StreamChunk, StreamSink, stream_explanation as stream_explanation_core,
};

struct TauriChannelSink(Channel<StreamChunk>);

impl StreamSink for TauriChannelSink {
    fn send(&self, chunk: StreamChunk) -> Result<(), String> {
        self.0.send(chunk).map_err(|err| err.to_string())
    }
}

pub async fn stream_explanation(req: QueryRequest, channel: Channel<StreamChunk>) {
    stream_explanation_core(req, Arc::new(TauriChannelSink(channel))).await;
}
