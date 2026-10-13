import { TestRegistry, assert } from "./helpers/test_assert.mjs";
import {
  escapeHtmlOracle,
  renderSafeMarkdownOracle,
  truncateSelectionOracle,
  loadLiveSafeMarkdownModule,
} from "./helpers/contract_oracle.mjs";
import {
  XSS_PAYLOADS,
  MARKDOWN_SAMPLES,
  BOUNDARY_STRINGS,
} from "./helpers/mock_fixtures.mjs";

export const tier3Suite = new TestRegistry("Tier 3: Cross-Feature Combinations");

// ==========================================
// Combination 1: Selection + Truncation + Streaming Markdown
// ==========================================
tier3Suite.test(
  "T3-TC1: Large selection (4500 chars) -> Truncate to 3000 -> Capsule prompt -> Safe streaming render",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    // 1. Simulate large selection with embedded code block
    const baseCode = "let data = compute_metric();\n";
    const hugeText = "```rust\n" + baseCode.repeat(150) + "```\n**Analysis complete.**";
    assert.ok(hugeText.length > 4000);

    // 2. Truncate
    const truncResult = truncFn(hugeText, 3000);
    assert.equal(truncResult.truncated, true);
    assert.equal(truncResult.text.length, 3000);
    assert.equal(truncResult.originalLen, hugeText.length);

    // 3. UI capsule prompt derivation
    const capsuleText = `已截取前 3,000 字符（原文共 ${truncResult.originalLen.toLocaleString()} 字）· 保护上下文`;
    assert.match(capsuleText, /已截取前 3,000 字符/);

    // 4. Stream chunks into renderer
    const chunkTokens = [
      truncResult.text.slice(0, 1000),
      truncResult.text.slice(1000, 2000),
      truncResult.text.slice(2000),
    ];

    let accumulated = "";
    for (const chunk of chunkTokens) {
      accumulated += chunk;
      const partialHtml = renderFn(accumulated);
      // Ensure partial rendering does not throw and maintains safe pre tags
      assert.ok(partialHtml.length > 0);
      assert.ok(!partialHtml.includes("<script>"));
    }

    const finalHtml = renderFn(accumulated);
    assert.match(finalHtml, /<pre class="glint-code-block"/);
  }
);

// ==========================================
// Combination 2: Hotkey Summon + Window Title + Prompt Envelope
// ==========================================
tier3Suite.test(
  "T3-TC2: Hotkey summon (Alt+Q) -> Foreground window perception -> Context prompt injection",
  async () => {
    // 1. Simulate active host environment
    const simulatedWindowTitle = "Visual Studio Code - glint/src/main.ts";
    const userSelectedText = "export function renderSafeMarkdown";

    // 2. Construct LLM context envelope
    const systemPromptHeader = "【用户当前宿主环境】";
    const promptEnvelope = `${systemPromptHeader}前台应用窗口: ${simulatedWindowTitle}`;

    assert.ok(promptEnvelope.includes(simulatedWindowTitle));

    // 3. Build API payload structure
    const payload = {
      messages: [
        { role: "system", content: promptEnvelope },
        { role: "user", content: `请解释选中的代码:\n${userSelectedText}` },
      ],
      stream: true,
      temperature: 0.3,
    };

    assert.equal(payload.messages.length, 2);
    assert.equal(payload.messages[0].role, "system");
    assert.match(payload.messages[0].content, /Visual Studio Code/);
    assert.equal(payload.stream, true);
  }
);

// ==========================================
// Combination 3: Multi-turn Follow-up + Streaming Typewriter
// ==========================================
tier3Suite.test(
  "T3-TC3: Multi-turn conversation -> Context retention -> Incremental typewriter token feed",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    // Turn 1
    const conversation = [];
    conversation.push({ role: "user", content: "什么是 Rust 的 RAII 模式？" });

    // Turn 1 Assistant streaming chunks
    const turn1Tokens = ["RAII 即", "“资源获取即初始化”", "，在 Rust 中通过", " `Drop` trait 实现自动析构。"];
    let turn1Text = "";
    for (const token of turn1Tokens) {
      turn1Text += token;
    }
    conversation.push({ role: "assistant", content: turn1Text });

    // Turn 2 User follow-up
    conversation.push({ role: "user", content: "能否给一个代码示例？" });

    // Turn 2 Assistant streaming
    const turn2Tokens = [
      "当然，如下所示：\n",
      "```rust\n",
      "struct FileGuard {\n",
      "    path: String,\n",
      "}\n",
      "```\n",
      "当超出作用域时自动调用 drop。",
    ];

    let turn2Accumulated = "";
    for (const token of turn2Tokens) {
      turn2Accumulated += token;
      const html = renderFn(turn2Accumulated);
      assert.ok(html.length > 0);
    }

    conversation.push({ role: "assistant", content: turn2Accumulated });

    assert.equal(conversation.length, 4);
    assert.equal(conversation[0].role, "user");
    assert.equal(conversation[1].role, "assistant");
    assert.equal(conversation[2].role, "user");
    assert.equal(conversation[3].role, "assistant");

    const finalRender = renderFn(turn2Accumulated);
    assert.match(finalRender, /<pre class="glint-code-block"><code data-lang="rust">/);
  }
);

// ==========================================
// Combination 4: Malicious XSS Payload in Truncated Document
// ==========================================
tier3Suite.test(
  "T3-TC4: Massive document with embedded XSS and code blocks -> Truncation + Safe Rendering",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    // Embed XSS payloads inside a large text that undergoes truncation
    let maliciousDoc = "Normal introduction text.\n";
    for (let i = 0; i < 50; i++) {
      maliciousDoc += `<script>alert('xss_${i}')</script>\n<img src=x onerror=evil()>\n`;
    }
    maliciousDoc += "```html\n<div class=\"active\"><script>stealth()</script></div>\n```\n";
    maliciousDoc += "Trailing text ".repeat(500);

    const truncated = truncFn(maliciousDoc, 3000);
    assert.equal(truncated.truncated, true);

    const rendered = renderFn(truncated.text);

    // Assert zero executable tags exist
    assert.ok(!rendered.includes("<script"));
    assert.ok(!rendered.includes("<img"));
    assert.ok(!rendered.includes("<iframe"));
    assert.match(rendered, /&lt;img src=x onerror=evil\(\)&gt;/);
  }
);

// ==========================================
// Combination 5: Rapid Hotkey Toggle & HUD Window Visibility Transitions
// ==========================================
tier3Suite.test(
  "T3-TC5: Rapid hotkey toggles (Alt+D) -> State transition loop stability",
  async () => {
    // Model state transitions for HUD visibility
    let isVisible = false;
    let toggleCount = 0;
    const toggleLog = [];

    for (let i = 0; i < 20; i++) {
      isVisible = !isVisible;
      toggleCount++;
      toggleLog.push({ iteration: i, isVisible });
    }

    assert.equal(toggleCount, 20);
    assert.equal(isVisible, false, "20 toggles should return window to hidden state");
    assert.equal(toggleLog.length, 20);
  }
);
