export const XSS_PAYLOADS = [
  {
    name: "Classic script tag",
    input: "<script>alert('xss')</script>",
    forbiddenOutputs: ["<script>", "alert('xss')", "<script>alert('xss')</script>"],
  },
  {
    name: "Img onerror attribute",
    input: '<img src="x" onerror="alert(1)"/>',
    forbiddenOutputs: ['onerror="alert(1)"', "<img "],
  },
  {
    name: "SVG onload attribute",
    input: '<svg onload="alert(document.domain)"></svg>',
    forbiddenOutputs: ['onload="alert', "<svg "],
  },
  {
    name: "Iframe javascript pseudo-protocol",
    input: '<iframe src="javascript:alert(1)"></iframe>',
    forbiddenOutputs: ['src="javascript:', "<iframe "],
  },
  {
    name: "Details tag with ontoggle event",
    input: '<details open ontoggle="alert(1)"><summary>click</summary></details>',
    forbiddenOutputs: ['ontoggle="alert(1)"', "<details "],
  },
  {
    name: "Body or div onload/onerror event",
    input: '<div onmouseover="alert(\'hover\')">Hover me</div>',
    forbiddenOutputs: ['onmouseover="alert', "<div "],
  },
  {
    name: "Nested and malformed HTML tags",
    input: '<<SCRIPT>alert("nested");//<</SCRIPT>',
    forbiddenOutputs: ['<SCRIPT>alert("nested")', '<script>'],
  },
  {
    name: "HTML entities with quote breakout",
    input: '" onfocus="alert(1)" autofocus="',
    forbiddenOutputs: ['onfocus="alert(1)"', 'autofocus="'],
  },
];

export const MARKDOWN_SAMPLES = {
  simpleBold: {
    input: "This is **bold text** in Glint.",
    expectedHtmlSubstring: "<strong>bold text</strong>",
  },
  inlineCode: {
    input: "Run `const result = compute();` now.",
    expectedHtmlSubstring: '<code class="glint-inline-code">const result = compute();</code>',
  },
  inlineCodeWithSpecialChars: {
    input: "Check `x < 10 && y > 20` condition.",
    expectedHtmlSubstring: '<code class="glint-inline-code">x &lt; 10 &amp;&amp; y &gt; 20</code>',
  },
  fencedCodeBlock: {
    input: "```rust\nfn main() {\n    println!(\"Hello Glint!\");\n}\n```",
    expectedTag: "<pre",
    expectedLangClass: 'data-lang="rust"',
  },
  fencedCodeBlockWithHtmlInside: {
    input: "```html\n<div class=\"lens\">\n  <script>evil()</script>\n</div>\n```",
    expectedTag: "<pre",
    shouldContainEscaped: "&lt;div class=&quot;lens&quot;&gt;",
    shouldNotContainRaw: "<script>evil()</script>",
  },
  streamingUnclosedCodeBlock: {
    input: "```typescript\ninterface VectorLens {\n  opacity: number;",
    expectedTag: "<pre",
    expectedClosing: "</code></pre>",
  },
  nestedBoldAndInlineCode: {
    input: "**Important: use `cargo test` always**",
    expectedHtmlSubstring: "<strong>Important: use <code",
  },
};

export const BOUNDARY_STRINGS = {
  empty: "",
  singleChar: "A",
  underLimit2999: "x".repeat(2999),
  exactLimit3000: "x".repeat(3000),
  overLimit3001: "x".repeat(3001),
  massive50000: "Large document content... ".repeat(2000),
  unicodeEmojis: "✨🎨🚀🔍💡🛡️⚡💎🔮🌟",
  astralPlaneSurrogatePair: "𠮷野家".repeat(1000), // U+20BB7 requires surrogate pairs
  combinedEmojiText: "Glint 纯原生桌面伴生透视 🚀".repeat(150),
};

export const SYSTEM_THRESHOLDS = {
  MAX_IDLE_RAM_MB: 15.0,
  MAX_HOTKEY_LATENCY_MS: 20.0,
  TARGET_CLIPBOARD_RETRY_INTERVAL_MS: 15,
  MAX_CLIPBOARD_POLL_TIMEOUT_MS: 350,
};
