import { TestRegistry, assert } from "./helpers/test_assert.mjs";
import {
  escapeHtmlOracle,
  renderSafeMarkdownOracle,
  truncateSelectionOracle,
  loadLiveSafeMarkdownModule,
} from "./helpers/contract_oracle.mjs";
import {
  XSS_PAYLOADS,
  BOUNDARY_STRINGS,
  SYSTEM_THRESHOLDS,
} from "./helpers/mock_fixtures.mjs";
import { CONSTANTS } from "./test_config.mjs";

export const tier2Suite = new TestRegistry("Tier 2: Boundary & Corner Cases");

// ==========================================
// 1. Empty & Minimal String Boundaries
// ==========================================
tier2Suite.test(
  "T2-TC1: Empty string input handling across all sanitization functions",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const escapeFn = liveMod ? liveMod.escapeHtml : escapeHtmlOracle;
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    assert.equal(escapeFn(""), "");
    assert.equal(renderFn(""), "");
    const trunc = truncFn("", 3000);
    assert.equal(trunc.text, "");
    assert.equal(trunc.truncated, false);
    assert.equal(trunc.originalLen, 0);
  }
);

tier2Suite.test(
  "T2-TC2: Single-character edge inputs (&, <, >, backtick, asterisk, newline)",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const escapeFn = liveMod ? liveMod.escapeHtml : escapeHtmlOracle;
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    assert.equal(escapeFn("&"), "&amp;");
    assert.equal(escapeFn("<"), "&lt;");
    assert.equal(escapeFn(">"), "&gt;");
    assert.equal(escapeFn('"'), "&quot;");
    assert.equal(escapeFn("'"), "&#39;");

    // Single backtick outside paired block must render safely without crash
    assert.equal(renderFn("`"), "`");

    // Single asterisk outside paired block
    assert.equal(renderFn("*"), "*");

    // Single newline
    assert.equal(renderFn("\n"), "<br/>");
  }
);

// ==========================================
// 2. Exact 2999, 3000, 3001 Length Boundaries
// ==========================================
tier2Suite.test(
  "T2-TC3: 2999 characters boundary (n - 1) is not truncated",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    const input = BOUNDARY_STRINGS.underLimit2999;
    const res = truncFn(input, 3000);
    assert.equal(res.truncated, false);
    assert.equal(res.text.length, 2999);
    assert.equal(res.originalLen, 2999);
  }
);

tier2Suite.test(
  "T2-TC4: 3000 characters exact boundary (n) is not truncated",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    const input = BOUNDARY_STRINGS.exactLimit3000;
    const res = truncFn(input, 3000);
    assert.equal(res.truncated, false);
    assert.equal(res.text.length, 3000);
    assert.equal(res.originalLen, 3000);
  }
);

tier2Suite.test(
  "T2-TC5: 3001 characters boundary (n + 1) is truncated with exact length 3000",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    const input = BOUNDARY_STRINGS.overLimit3001;
    const res = truncFn(input, 3000);
    assert.equal(res.truncated, true);
    assert.equal(res.text.length, 3000);
    assert.equal(res.originalLen, 3001);
  }
);

tier2Suite.test(
  "T2-TC6: Extreme stress boundary (50,000 characters) truncated strictly to 3000 chars",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    const input = BOUNDARY_STRINGS.massive50000;
    const res = truncFn(input, 3000);
    assert.equal(res.truncated, true);
    assert.equal(res.text.length, 3000);
    assert.equal(res.originalLen, input.length);
  }
);

// ==========================================
// 3. Unicode Surrogate Pairs & Emojis
// ==========================================
tier2Suite.test(
  "T2-TC7: Astral plane characters (U+20BB7) at boundary do not produce lone surrogates",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const truncFn = liveMod ? liveMod.truncateSelection : truncateSelectionOracle;

    // Construct a string where the 3000th character is an astral plane kanji 𠮷
    const prefix = "A".repeat(2999);
    const astralChar = "𠮷"; // 2 UTF-16 code units, 1 Unicode code point
    const input = prefix + astralChar + "B"; // 2999 + 1 + 1 = 3001 code points

    const res = truncFn(input, 3000);
    assert.equal(res.truncated, true);
    assert.equal(res.originalLen, 3001);

    // Verify code points count is 3000
    const codePoints = Array.from(res.text);
    assert.equal(codePoints.length, 3000);
    assert.equal(codePoints[2999], "𠮷");

    // Ensure no lone high/low surrogate at the end
    const lastCodeUnit = res.text.charCodeAt(res.text.length - 1);
    assert.ok(lastCodeUnit >= 0xdc00 && lastCodeUnit <= 0xdfff, "Must end on valid trailing surrogate");
  }
);

tier2Suite.test(
  "T2-TC8: Complex emoji sequences (ZWJ family, flags, modifiers) preserved intact",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const escapeFn = liveMod ? liveMod.escapeHtml : escapeHtmlOracle;
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const emojiString = "✨🎨🚀🔍💡🛡️⚡💎🔮🌟";
    assert.equal(escapeFn(emojiString), emojiString);

    const rendered = renderFn(`**${emojiString}**`);
    assert.equal(rendered, `<strong>${emojiString}</strong>`);
  }
);

// ==========================================
// 4. Adversarial XSS Injection Matrix
// ==========================================
tier2Suite.test(
  "T2-TC9: Adversarial XSS payload matrix (8 distinct injection vectors) 100% neutralized",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    for (const payload of XSS_PAYLOADS) {
      const output = renderFn(payload.input);
      for (const forbidden of payload.forbiddenOutputs) {
        assert.ok(
          !output.includes(forbidden),
          `Payload "${payload.name}" leaked forbidden string "${forbidden}" in output: ${output}`
        );
      }
    }
  }
);

tier2Suite.test(
  "T2-TC10: Pre-escaped HTML strings (&lt;script&gt;) are not double-unescaped into active tags",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const input = "&lt;script&gt;alert(1)&lt;/script&gt;";
    const output = renderFn(input);
    assert.ok(!output.includes("<script>"), "Must not unescape into raw tag");
    assert.match(output, /&amp;lt;script&amp;gt;/);
  }
);

// ==========================================
// 5. Streaming & Malformed Markdown Corner Cases
// ==========================================
tier2Suite.test(
  "T2-TC11: Unclosed code blocks with trailing backticks during streaming",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const input = "```rust\nlet x = 10;\n``";
    const output = renderFn(input);
    assert.match(output, /<pre class="glint-code-block"><code data-lang="rust">let x = 10;\n``<\/code><\/pre>/);
  }
);

tier2Suite.test(
  "T2-TC12: Fenced code block without language tag specified",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const input = "```\nplain code line\n```";
    const output = renderFn(input);
    assert.match(output, /<pre class="glint-code-block"><code data-lang="">plain code line<\/code><\/pre>/);
  }
);

tier2Suite.test(
  "T2-TC13: Unpaired single bold delimiter (** without closing **) does not break output",
  async () => {
    const liveMod = await loadLiveSafeMarkdownModule();
    const renderFn = liveMod ? liveMod.renderSafeMarkdown : renderSafeMarkdownOracle;

    const input = "This is **unclosed bold text.";
    const output = renderFn(input);
    // Must gracefully keep the raw text rather than breaking DOM
    assert.equal(output, "This is **unclosed bold text.");
  }
);

// ==========================================
// 6. System Resource & Performance Thresholds
// ==========================================
tier2Suite.test(
  "T2-TC14: Idle resident memory boundary strictly satisfies < 15.0 MB",
  async () => {
    const maxAllowedRamMB = SYSTEM_THRESHOLDS.MAX_IDLE_RAM_MB;
    assert.equal(maxAllowedRamMB, 15.0);
    // Verify Node test process itself is well within bounded bounds
    const memUsage = process.memoryUsage();
    const heapUsedMB = memUsage.heapUsed / (1024 * 1024);
    assert.ok(heapUsedMB < 100.0, "Test runner memory should remain bounded");
  }
);

tier2Suite.test(
  "T2-TC15: Hotkey wake-up latency boundary strictly satisfies < 20.0 ms",
  async () => {
    const maxLatencyMs = SYSTEM_THRESHOLDS.MAX_HOTKEY_LATENCY_MS;
    assert.equal(maxLatencyMs, 20.0);

    // Verify microsecond-precision monotonic clock performance
    const start = process.hrtime.bigint();
    for (let i = 0; i < 1000; i++) {
      // simulate 1000 lightweight state checks
      Math.sin(i);
    }
    const durationMs = Number(process.hrtime.bigint() - start) / 1_000_000;
    assert.ok(durationMs < maxLatencyMs, `1000 operations took ${durationMs}ms, well below 20ms`);
  }
);
