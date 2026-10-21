//! Headless harness for the Glint core.
//!
//! This binary proves the core engines on the real desktop without involving a
//! GUI: session detection, PRIMARY capture, KGlobalAccel/portal registration,
//! and streaming output.

use std::sync::Arc;

use glint_core::{
    ChatMessage, HotkeyAction, HotkeyCallback, PlatformHotkeyEngine, QueryRequest, StreamChunk,
    StreamSink, default_bindings, detect_session, is_kde, load_default_config, platform_sensor,
    stream_explanation,
};

struct StdoutSink;

impl StreamSink for StdoutSink {
    fn send(&self, chunk: StreamChunk) -> Result<(), String> {
        use std::io::Write;

        let mut stdout = std::io::stdout();
        match chunk {
            StreamChunk::Token(token) => {
                stdout
                    .write_all(token.as_bytes())
                    .map_err(|err| err.to_string())?;
                stdout.flush().map_err(|err| err.to_string())?;
            }
            StreamChunk::ToolCall(tool) => {
                eprintln!("\n[tool {} · {}]", tool.name, tool.status);
            }
            StreamChunk::Done => {
                println!();
            }
            StreamChunk::Error(message) => {
                eprintln!("\n[stream error] {message}");
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "help".to_string());

    match command.as_str() {
        "session" => {
            println!("session: {}", detect_session().as_str());
            println!("kde: {}", is_kde());
            println!(
                "XDG_SESSION_TYPE: {}",
                std::env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "-".to_string())
            );
            println!(
                "XDG_CURRENT_DESKTOP: {}",
                std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "-".to_string())
            );
            println!(
                "WAYLAND_DISPLAY: {}",
                std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "-".to_string())
            );
            println!(
                "DISPLAY: {}",
                std::env::var("DISPLAY").unwrap_or_else(|_| "-".to_string())
            );
        }
        "screen" => {
            // Verify the portal really returns the whole virtual desktop: the
            // snip overlay is sized to that same bounding box.
            match glint_core::capture_via_portal().await {
                Ok(frame) => {
                    println!("capture: {}x{}", frame.width, frame.height);
                    println!("data_url_bytes: {}", frame.image_data_url.len());
                    // Optional output path for visual inspection: the portal
                    // deletes its temp file, so keep a copy on request.
                    if let Some(path) = std::env::args().nth(2) {
                        use base64::Engine;
                        let encoded = frame
                            .image_data_url
                            .strip_prefix("data:image/png;base64,")
                            .unwrap_or("");
                        let bytes = base64::engine::general_purpose::STANDARD
                            .decode(encoded)
                            .map_err(|err| err.to_string())?;
                        std::fs::write(&path, bytes)?;
                        println!("wrote: {path}");
                    }
                }
                Err(err) => eprintln!("capture failed: {err}"),
            }
        }
        "capture" => {
            let candidate = platform_sensor().capture()?;
            println!("source: {:?}", candidate.source);
            println!(
                "chars: {} (original: {}, truncated: {})",
                candidate.text.chars().count(),
                candidate.original_len,
                candidate.truncated
            );
            println!("---");
            println!("{}", candidate.text);
        }
        "explain" => {
            let candidate = platform_sensor().capture()?;
            let config = load_default_config();

            let messages = vec![ChatMessage::user_text(format!(
                "请解释下面这段选区内容：先给一句结论，再说明关键机制与权衡。\n\n{}",
                candidate.text
            ))];

            let request = QueryRequest {
                messages,
                api_key: if config.api_key.trim().is_empty() {
                    None
                } else {
                    Some(config.api_key)
                },
                base_url: Some(config.base_url),
                model: Some(config.model),
                // CLI is a debugging harness: allow one tool round on demand.
                enable_tools: std::env::args().any(|arg| arg == "--tools"),
                window_title: None,
            };

            eprintln!(
                "[glint] selection from {:?}, {} chars; streaming...",
                candidate.source,
                candidate.text.chars().count()
            );
            stream_explanation(request, Arc::new(StdoutSink)).await;
        }
        "hotkeys" => {
            let engine = PlatformHotkeyEngine::new("com.glint.desktop", "Glint");
            let bindings = default_bindings();
            let callback: HotkeyCallback = Arc::new(|action: HotkeyAction| {
                println!("[hotkey] {action:?}");
            });

            engine.register(bindings.clone(), callback).await?;
            println!(
                "registered {} shortcuts; press Ctrl-C to unregister",
                bindings.len()
            );
            for binding in &bindings {
                println!(
                    "  {} -> {:?} (qt key 0x{:X})",
                    binding.id,
                    binding.action,
                    binding.key.to_qt_key()
                );
            }

            tokio::signal::ctrl_c().await?;
            engine.unregister(&bindings).await?;
            println!("unregistered");
        }
        "help" | "--help" | "-h" => {
            println!("glint-cli <command>");
            println!();
            println!("  session    show session type and desktop");
            println!("  capture    read the PRIMARY selection and print it");
            println!("  explain    capture the selection and stream an explanation");
            println!("  hotkeys    register KGlobalAccel/portal shortcuts and wait");
        }
        other => {
            eprintln!("unknown command: {other}");
            eprintln!("try: glint-cli help");
            std::process::exit(2);
        }
    }

    Ok(())
}
