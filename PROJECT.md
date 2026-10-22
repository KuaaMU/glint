# Project: Glint Pure Native Desktop Vector Lens & Security Hardening

> **Status note (2026-10-19):** This document is a target specification, not an
> implementation report. `crates/glint-hud` does not exist; M2–M4 are not
> implemented. The current Linux direction is recorded in
> `docs/adr/0012-linux-native-companion-architecture.md`, and the real test
> status is in `TEST_READY.md`.

## Architecture
Glint evolves along a dual-track architecture:
1. **Existing Web Track (Hardened)**:
   - Frontend: Vite + TypeScript (`src/main.ts`, `src/preferences.ts`, `src/snipping.ts`).
   - Hardening: Zero-dependency HTML entity escaping and streaming-resilient Markdown parser for code blocks (` ``` `), inline code (` ` `), and bold text (`**`). Zero naked `innerHTML`. Unicode-safe 3000-character input truncation with amber visual capsule prompt.
   - Backend: Tauri v2 (`src-tauri`) providing system tray and legacy webview companion interface.
2. **Pure Native HUD Track (Slint + Pure Rust)**:
   - Architecture: Standalone Cargo workspace crate `crates/glint-hud` producing `glint-hud.exe`.
   - GUI Framework: Slint 1.18.1 using `renderer-femtovg` (pure Rust OpenGL hardware-accelerated vector rendering, ~8-12MB working set RAM, zero WebView2/HTML/CSS/Node.js dependencies).
   - Visual Aesthetics: Obsidian acrylic glass texture, DWM backdrop blur via Win32 FFI, 1px gradient light blade, 0.5px hairline craftsmanship, breathing glow animation, borderless transparent window (`no-frame: true`).
   - Performance: Hotkey wake-up latency < 20ms (Win32 `SetWindowPos` < 3ms), idle resident RAM < 15MB.
3. **Shared Core Engine**:
   - Win32 Zero-Conflict Clipboard Sequence Engine (`capture.rs` + `platform/windows.rs`, `GetClipboardSequenceNumber` kernel polling + modifier synthesis).
   - Foreground Window Perception Hook (`GetForegroundWindow` + `GetWindowTextW`).
   - Multimodal Streaming LLM Channel (`reqwest` SSE client + offline fallback typewriter engine) decoupled with generic `StreamSink` channel interface.
   - Global Hotkey Message Loop (`RegisterHotKey` + `GetMessageW` on dedicated thread).

```
                        ┌──────────────────────────────────────────────┐
                        │              Glint Workspace                 │
                        └───────┬──────────────────────────────┬───────┘
                                │                              │
                ┌───────────────▼──────────────┐ ┌─────────────▼────────────────┐
                │        src-tauri             │ │      crates/glint-hud        │
                │  (Tauri v2 App + glint_lib)  │ │   (Pure Native Slint HUD)    │
                └───────────────┬──────────────┘ └─────────────┬────────────────┘
                                │                              │
                        ┌───────▼──────────────────────────────▼───────┐
                        │             Shared Core Engines              │
                        │  - Win32 Clipboard Sequence (capture.rs)     │
                        │  - Window Title Perception (windows.rs)      │
                        │  - Streaming LLM Pipeline (llm.rs)           │
                        │  - Global Win32 Hotkey Loop (windows.rs)     │
                        └──────────────────────────────────────────────┘
```

---

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | Safe Markdown Parser | Zero-dependency safe HTML escape & streaming parser for code blocks, inline code, bold text | M1 | ORIGINAL_REQUEST §R1 |
| 2 | Eliminate `innerHTML` | Eradicate all 7 naked `innerHTML` usages in `src/main.ts` with DOM-safe rendering | M1 | ORIGINAL_REQUEST §R1 |
| 3 | 3000-char Truncation | Truncate selected text > 3000 chars with amber UI capsule warning to prevent context overflow | M1 | ORIGINAL_REQUEST §R1 |
| 4 | README Alignment | Accurately document Windows Tier-1 vs macOS/Linux experimental stubs, document `Alt + W` hotkey | M1 | ORIGINAL_REQUEST §R1 |
| 5 | Slint Workspace & Crate | Cargo workspace setup with `crates/glint-hud` and Slint 1.18.1 (`renderer-femtovg`) | M2 | ORIGINAL_REQUEST §R2 |
| 6 | Obsidian Glass Texture | Borderless transparent window with DWM acrylic blur & dark crystal gradient aesthetics | M2 | ORIGINAL_REQUEST §R2 |
| 7 | Precision Vector Lines | 1px gradient light blade border and 0.5px precision line-art styling | M2 | ORIGINAL_REQUEST §R2 |
| 8 | Breathing Glow Animation | Hardware-accelerated continuous breathing glow and smooth visual transitions | M2 | ORIGINAL_REQUEST §R2 |
| 9 | Zero Web Runtime | Zero WebView2, HTML, CSS, or Node.js runtime footprint in native HUD | M2 | ORIGINAL_REQUEST §R2 |
| 10 | Clipboard Engine Reuse | Reuse Win32 zero-conflict clipboard sequence polling (`GetClipboardSequenceNumber`) | M3 | ORIGINAL_REQUEST §R3 |
| 11 | Window Title Awareness | Inject active foreground window title (`GetForegroundWindow`) into HUD LLM context | M3 | ORIGINAL_REQUEST §R3 |
| 12 | Streaming LLM Channel | Multi-turn streaming token feed decoupled from Tauri into native Slint UI | M3 | ORIGINAL_REQUEST §R3 |
| 13 | Hotkey HUD Summon | Global `Alt + Q`, `Alt + D` summon HUD with < 20ms invocation latency | M3 | ORIGINAL_REQUEST §R3 |
| 14 | Typewriter Text Display | Smooth streaming typewriter token display in Slint native view | M3 | ORIGINAL_REQUEST §R3 |
| 15 | Multi-Turn Follow-up | Interactive text input in native HUD for deep conversational follow-up questions | M3 | ORIGINAL_REQUEST §R3 |
| 16 | Idle RAM < 15MB Verification | Verified idle resident memory strictly below 15MB via Win32 PSAPI automated tests | M4 | ORIGINAL_REQUEST Acceptance |
| 17 | XSS Injection Defense Tests | Verified zero XSS execution against `<img onerror>`, `<script>`, `<svg>` payloads | M4 | ORIGINAL_REQUEST Acceptance |
| 18 | Dual-Track Non-Interference | All 14 existing `cargo test` and `npm run build` pass cleanly without regression | M4 | ORIGINAL_REQUEST Acceptance |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M1 | Existing Version Safety & Typography Hardening | Eliminate `innerHTML` in `src/main.ts`, zero-dep safe markdown parser, 3000-char truncation capsule, update `README.md` | none | IN_PROGRESS |
| M2 | Slint Pure Native HUD Architecture | Cargo workspace setup, `crates/glint-hud`, Obsidian acrylic glass window, 1px/0.5px line art, breathing glow, borderless transparency | none | PLANNED |
| M3 | Core Rust Engine Reuse & Session Integration | Decouple core engines from Tauri, Win32 clipboard sequence, `window_title`, streaming LLM channel, hotkeys, typewriter text, multi-turn follow-up | M2 | PLANNED |
| M4 | Final E2E Test Suite & Adversarial Verification | Pass 100% of E2E test suite (Tiers 1-4) published in `TEST_READY.md`, Tier 5 adversarial coverage hardening | M1, M2, M3 | PLANNED |

---

## Interface Contracts

### 1. Frontend Markdown & Sanitization Contract (`src/safe-markdown.ts`)
- `escapeHtml(raw: string): string`: Converts `&`, `<`, `>`, `"`, `'` to HTML character entities.
- `renderSafeMarkdown(text: string): string`: Escapes input HTML first, then transforms:
  - Fenced code blocks: ` ```[lang]\n[code]\n``` ` -> `<pre class="glint-code-block"><code data-lang="...">...</code></pre>`
  - Streaming open code blocks: ` ```[lang]\n[code] ` -> handles open state gracefully.
  - Inline code: `` `[code]` `` -> `<code class="glint-inline-code">...</code>`.
  - Bold: `**[text]**` -> `<strong>...</strong>`.
  - Newlines: single newline -> `<br/>`, double newline -> `<br/><br/>`.
- `truncateSelection(text: string, maxLen?: number): { text: string; truncated: boolean; originalLen: number }`:
  - Unicode character limit (default 3000). Returns truncated string and metadata flag.

### 2. Rust Core Engine Shared Interface (`src-tauri/src/core_bridge.rs` or `glint_lib`)
- Trait `StreamSink`:
  ```rust
  pub trait StreamSink: Send + Sync + 'static {
      fn emit_chunk(&self, chunk: StreamChunk) -> Result<(), String>;
  }
  ```
- Function `stream_explanation_generic(req: QueryRequest, sink: Arc<dyn StreamSink>) -> impl Future<Output = Result<(), String>>`:
  - Decoupled from `tauri::ipc::Channel`, accepts any `StreamSink` (e.g. Tokio mpsc for Slint, Tauri IPC for Tauri).
- Win32 Hotkey Hook Callback:
  ```rust
  pub enum HotkeyAction {
      TriggerSelection, // Alt + Q
      ToggleVisibility, // Alt + D
      TriggerSnipping,  // Alt + W
  }
  pub fn spawn_native_hotkeys<F: Fn(HotkeyAction) + Send + 'static>(callback: F) -> std::thread::JoinHandle<()>;
  ```

### 3. Slint HUD UI Model Contract (`crates/glint-hud/ui/hud.slint`)
- Global / Window Properties:
  - `property <string> host-window-title: ""`
  - `property <string> selected-text-preview: ""`
  - `property <string> streaming-response-text: ""`
  - `property <bool> is-streaming: false`
  - `property <bool> has-capsule-warning: false`
  - `property <string> capsule-warning-text: ""`
  - `callback submit-followup(string)`
  - `callback close-hud()`
  - `callback toggle-pin()`

---

## Code Layout
```
F:\glint\
├── Cargo.toml                       # Root Cargo Workspace manifest
├── package.json                     # NPM scripts (dev, build, preview)
├── README.md                        # Documentation (Windows Tier-1, Alt+W, architecture)
├── src\
│   ├── main.ts                      # Hardened webview companion logic
│   ├── safe-markdown.ts             # Zero-dependency safe HTML escape + Markdown parser
│   ├── safe-markdown.test.ts        # Node test suite for escaping & markdown
│   ├── preferences.ts               # Preferences management
│   └── snipping.ts                  # Snipping tool
├── src-tauri\                       # Existing Tauri backend
│   ├── Cargo.toml                   # glint_lib crate
│   ├── src\
│   │   ├── lib.rs                   # Tauri plugin setup & command dispatch
│   │   ├── capture.rs               # Win32 clipboard sequence engine
│   │   ├── llm.rs                   # Multimodal streaming LLM client & fallback
│   │   ├── core_bridge.rs           # Generic StreamSink & channel abstractions
│   │   └── platform\
│   │       ├── mod.rs               # Platform dispatch
│   │       └── windows.rs           # Win32 FFI (hotkeys, window_title, sequence)
└── crates\
    └── glint-hud\                   # Pure Native Slint HUD
        ├── Cargo.toml               # Slint 1.18.1 (renderer-femtovg)
        ├── build.rs                 # slint_build::compile("ui/hud.slint")
        ├── src\
        │   ├── main.rs              # HUD entrypoint, event loop, Win32 window setup
        │   ├── window.rs            # Borderless transparency, DWM acrylic blur
        │   ├── session.rs           # Multi-turn conversational session state
        │   └── bridge.rs            # Bridge between core Rust engine and Slint event loop
        └── ui\
            ├── hud.slint            # Main HUD window: acrylic glass, 1px/0.5px line art
            ├── components\          # Slint UI components (token view, capsule, input)
            └── style.slint          # Obsidian colors, animations, borders
```
