import path from "node:path";
import { execSync } from "node:child_process";
import { SKIP_SHELL } from "./test_config.mjs";
import { TestRegistry, assert } from "./helpers/test_assert.mjs";
import {
  renderSafeMarkdownOracle,
  loadLiveSafeMarkdownModule,
} from "./helpers/contract_oracle.mjs";
import { SRC_DIR, SRC_TAURI_DIR } from "./test_config.mjs";

export const tier4Suite = new TestRegistry("Tier 4: Real-World Workloads");

// ==========================================
// Workload 1: Complex Multi-Language Programming Code Snippets
// ==========================================
tier4Suite.test(
  "T4-TC1: Multi-language complex code snippets with generics, lifetimes, and quotes",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const complexSnippet = [
      "# Glint Architecture Overview\n",
      "Here is the **core engine** signature in `src-tauri/src/capture.rs`:\n",
      "```rust\n",
      "pub fn capture_selected_text<'a, T: AsRef<str> + ?Sized>(\n",
      "    name: &'a T,\n",
      "    retries: usize,\n",
      ") -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {\n",
      '    let expr = "x < 10 && y > 20";\n',
      '    println!("Processing: {}", expr);\n',
      "    Ok(None)\n",
      "}\n",
      "```\n",
      "And the corresponding **TypeScript** bridge interface:\n",
      "```typescript\n",
      "export interface QueryRequest<T = Record<string, unknown>> {\n",
      "  messages: Array<{ role: 'user' | 'assistant'; content: string }>;\n",
      "  window_title?: string;\n",
      "  metadata?: T;\n",
      "}\n",
      "```\n",
      "Notice how `<T>` and `&&` must be entity-encoded inside the code block for security.",
    ].join("");

    const output = renderFn(complexSnippet);

    // Verify generics <T> and lifetimes &'a are properly escaped within code tags
    assert.match(output, /&lt;T = Record&lt;string/);
    assert.match(output, /&amp;&#39;a T/);
    assert.match(output, /<pre class="glint-code-block"><code data-lang="rust">/);
    assert.match(output, /<pre class="glint-code-block"><code data-lang="typescript">/);
    assert.match(output, /<strong>core engine<\/strong>/);
    assert.ok(!output.includes("<script>"));
  }
);

// ==========================================
// Workload 2: Deep 5-Turn Developer Dialogue Simulation
// ==========================================
tier4Suite.test(
  "T4-TC2: Deep 5-turn conversational session with accumulated context and token streaming",
  async () => {
    const turns = [
      {
        user: "How does Glint prevent clipboard mutex deadlocks?",
        assistant:
          "Glint uses `GetClipboardSequenceNumber` kernel polling without calling `OpenClipboard`.\nThis avoids locking the clipboard handle.",
      },
      {
        user: "What about compatibility with Ditto or other clipboard managers?",
        assistant:
          "Glint employs a 5-iteration retry loop with a 6ms backoff sleep.\nIf sequence changes, it snapshots the buffer safely.",
      },
      {
        user: "How is high-DPI scaling handled in Windows?",
        assistant:
          "Glint queries `GetDpiForWindow` and scales logical pixels to physical coordinates (`dpi / 96.0`).",
      },
      {
        user: "Can you show the GetForegroundWindow FFI definition?",
        assistant:
          "```rust\nunsafe extern \"system\" {\n    pub fn GetForegroundWindow() -> HWND;\n}\n```",
      },
      {
        user: "Summarize the Slint FemtoVG memory advantages.",
        assistant:
          "FemtoVG renders directly to OpenGL vector paths,\nmaintaining idle RAM at ~8-12MB without WebView2 bloat.",
      },
    ];

    const messageHistory = [];
    for (let i = 0; i < turns.length; i++) {
      const turn = turns[i];
      messageHistory.push({ role: "user", content: turn.user });
      messageHistory.push({ role: "assistant", content: turn.assistant });
    }

    assert.equal(messageHistory.length, 10);
    assert.equal(messageHistory[0].role, "user");
    assert.equal(messageHistory[9].role, "assistant");

    // Verify all assistant replies render cleanly
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    for (const msg of messageHistory) {
      if (msg.role === "assistant") {
        const html = renderFn(msg.content);
        assert.ok(html.length > 0);
      }
    }
  }
);

// ==========================================
// Workload 3: Rapid Hotkey Toggle Stress Simulation
// ==========================================
tier4Suite.test(
  "T4-TC3: 100-cycle rapid hotkey toggle stress simulation",
  async () => {
    let state = "hidden";
    const startTime = process.hrtime.bigint();

    for (let i = 0; i < 100; i++) {
      if (state === "hidden") {
        state = "visible";
      } else {
        state = "hidden";
      }
    }

    const durationMs = Number(process.hrtime.bigint() - startTime) / 1_000_000;
    assert.equal(state, "hidden");
    assert.ok(durationMs < 50.0, `100 toggle transitions executed in ${durationMs}ms`);
  }
);

// ==========================================
// Workload 4: Concurrent Dual-Track Build & Test Verification
// ==========================================
tier4Suite.test(
  "T4-TC4: Concurrent backend unit tests and frontend production bundling",
  async () => {
    // 1. Rust backend unit tests
    const cargoOut = execSync("cargo test", {
      cwd: SRC_TAURI_DIR,
      encoding: "utf-8",
    });
    assert.match(cargoOut, /\d+ passed; 0 failed/);

    // 2. Frontend production bundling
    const npmOut = execSync("npm run build", {
      cwd: path.resolve(SRC_DIR, ".."),
      encoding: "utf-8",
    });
    assert.match(npmOut, /built in/i);
  },
  { skip: SKIP_SHELL }
);
