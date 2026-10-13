/**
 * Empirical Adversarial Challenger Test Suite: Streaming Markdown & Truncation
 * Milestone 1: R1 Safety & Typography Hardening
 *
 * Authored and independently verified by teamwork_preview_challenger_m1_2.
 */

import {
  escapeHtml,
  parseInlineTokens,
  renderSafeMarkdown,
  truncateSelection,
} from "../src/safe-markdown.ts";

let totalTests = 0;
let passedTests = 0;
let failedTests = 0;
const failures = [];

function assert(condition, message) {
  if (!condition) {
    throw new Error(`Assertion Failed: ${message}`);
  }
}

function assertEqual(actual, expected, message) {
  if (actual !== expected) {
    throw new Error(`${message || "Value mismatch"} - Expected: ${JSON.stringify(expected)}, Got: ${JSON.stringify(actual)}`);
  }
}

function test(name, fn) {
  totalTests++;
  try {
    fn();
    console.log(`  [✓ PASS] ${name}`);
    passedTests++;
  } catch (err) {
    console.error(`  [✗ FAIL] ${name}`);
    console.error(`     Error: ${err.message}`);
    failures.push({ name, error: err.message });
    failedTests++;
  }
}

/**
 * Validates HTML structural integrity:
 * - Checks that all tags opened are properly closed in valid LIFO order
 * - Allows self-closing <br/> or <br>
 * - Ensures no naked unescaped '<' or '>' remain
 */
function validateHtmlWellFormedness(html) {
  // Strip authorized tags and check balance
  const tagRegex = /<\/?([a-zA-Z0-9_-]+)(?:\s+[^>]*?)?\/?>/g;
  const stack = [];
  let match;

  while ((match = tagRegex.exec(html)) !== null) {
    const fullTag = match[0];
    const tagName = match[1].toLowerCase();

    // Self-closing tags
    if (fullTag.endsWith("/>") || tagName === "br" || tagName === "img") {
      continue;
    }

    if (fullTag.startsWith("</")) {
      // Closing tag
      if (stack.length === 0) {
        throw new Error(`Unexpected closing tag </${tagName}> without matching open tag in: ${html}`);
      }
      const top = stack.pop();
      if (top !== tagName) {
        throw new Error(`Mismatched closing tag </${tagName}>, expected </${top}> in: ${html}`);
      }
    } else {
      // Opening tag
      stack.push(tagName);
    }
  }

  if (stack.length > 0) {
    throw new Error(`Unclosed tag(s) remaining: [${stack.join(", ")}] in: ${html}`);
  }
}

console.log("================================================================");
console.log("  ADVERSARIAL STRESS TEST: STREAMING SIMULATION & TRUNCATION");
console.log("================================================================");

// -----------------------------------------------------------------------------
// Suite 1: Token-by-Token Streaming Simulation
// -----------------------------------------------------------------------------
console.log("\n--- Suite 1: Token-by-Token Streaming Simulation ---");

test("1.1 Streaming open code block: ```rust\\nfn main() { token-by-token", () => {
  const fullText = "```rust\nfn main() {\n    let answer = 42;\n    println!(\"{}\", answer);\n}";
  let accumulated = "";

  for (let i = 0; i < fullText.length; i++) {
    accumulated += fullText[i];
    const rendered = renderSafeMarkdown(accumulated);

    // Assert well-formed HTML at every single character step
    try {
      validateHtmlWellFormedness(rendered);
    } catch (err) {
      throw new Error(`At step ${i} ('${accumulated.replace(/\n/g, "\\n")}'): ${err.message}`);
    }

    // When the fence is complete (>= 3 chars), it must be inside a <pre><code>
    if (i >= 2 && accumulated.startsWith("```")) {
      assert(rendered.includes('<pre class="glint-code-block">'), `Expected pre block at step ${i}`);
      assert(rendered.includes('</code></pre>'), `Expected properly closed pre block at step ${i}`);
    }
  }

  // Final check
  const finalRendered = renderSafeMarkdown(fullText);
  assert(finalRendered.includes('data-lang="rust"'), "Must detect rust language");
  assert(finalRendered.includes("let answer = 42;"), "Must contain code body");
});

test("1.2 Streaming code block closure and transition to post-code text", () => {
  const fullText = "```rust\nfn main() {}\n```\nHere is explanation text after code.";
  let accumulated = "";

  for (let i = 0; i < fullText.length; i++) {
    accumulated += fullText[i];
    const rendered = renderSafeMarkdown(accumulated);
    validateHtmlWellFormedness(rendered);
  }

  const finalRendered = renderSafeMarkdown(fullText);
  assert(finalRendered.includes('<pre class="glint-code-block"><code data-lang="rust">fn main() {}</code></pre>'), "Closed code block");
  assert(finalRendered.includes("Here is explanation text after code."), "Post-code text rendered");
});

test("1.3 Streaming inline code: `let x = 1;` token-by-token", () => {
  const fullText = "Here is an expression: `let x = 1;` which initializes x.";
  let accumulated = "";

  for (let i = 0; i < fullText.length; i++) {
    accumulated += fullText[i];
    const rendered = renderSafeMarkdown(accumulated);

    validateHtmlWellFormedness(rendered);

    // During unclosed inline code (before the closing backtick),
    // it must NOT leave an unclosed <code tag.
    const openBacktickIdx = accumulated.indexOf("`");
    const closeBacktickIdx = accumulated.indexOf("`", openBacktickIdx + 1);

    if (openBacktickIdx !== -1 && closeBacktickIdx === -1) {
      assert(!rendered.includes("<code"), `Must not emit open <code tag before closing backtick at step ${i}`);
    } else if (closeBacktickIdx !== -1) {
      assert(rendered.includes('<code class="glint-inline-code">let x = 1;</code>'), `Must emit closed inline code at step ${i}`);
    }
  }
});

test("1.4 Streaming unclosed bold: **hello world token-by-token", () => {
  const fullText = "Notice that **hello world is highlighted";
  let accumulated = "";

  for (let i = 0; i < fullText.length; i++) {
    accumulated += fullText[i];
    const rendered = renderSafeMarkdown(accumulated);

    validateHtmlWellFormedness(rendered);

    // Since bold is never closed in fullText, it must NEVER emit an unclosed <strong> tag
    assert(!rendered.includes("<strong"), `Must not emit dangling <strong> for unclosed bold at step ${i}`);
  }

  // Now append the closing **
  accumulated += "**";
  const closedRendered = renderSafeMarkdown(accumulated);
  validateHtmlWellFormedness(closedRendered);
  assert(closedRendered.includes("<strong>hello world is highlighted</strong>"), "Must emit <strong> when closed");
});

test("1.5 Streaming nested inline code inside bold token-by-token", () => {
  const fullText = "Important: **Use `npm run test` now** in CLI.";
  let accumulated = "";

  for (let i = 0; i < fullText.length; i++) {
    accumulated += fullText[i];
    const rendered = renderSafeMarkdown(accumulated);
    validateHtmlWellFormedness(rendered);
  }

  const finalRendered = renderSafeMarkdown(fullText);
  assert(
    finalRendered.includes('<strong>Use <code class="glint-inline-code">npm run test</code> now</strong>'),
    `Must properly nest inline code inside bold: ${finalRendered}`
  );
});

test("1.6 Alternating newlines and code chunks streaming", () => {
  const sequence = [
    "Paragraph 1 text.\n\n",
    "```typescript\n",
    "interface User {\n",
    "  id: number;\n",
    "  name: string;\n",
    "}\n",
    "```\n\n",
    "Paragraph 2 with `inline`.\n\n",
    "```bash\n",
    "cargo build --release\n",
    "```\n\n",
    "Final conclusion.",
  ];

  let accumulated = "";
  for (const chunk of sequence) {
    for (const char of chunk) {
      accumulated += char;
      const rendered = renderSafeMarkdown(accumulated);
      validateHtmlWellFormedness(rendered);
    }
  }

  const finalRendered = renderSafeMarkdown(accumulated);
  // Verify both code blocks exist
  assert(finalRendered.includes('data-lang="typescript"'), "Typescript block exists");
  assert(finalRendered.includes('data-lang="bash"'), "Bash block exists");
  assert(finalRendered.includes("Paragraph 1 text.<br/><br/>"), "Double newline before code block");
  assert(finalRendered.includes("Final conclusion."), "Final conclusion exists");
});

test("1.7 Adversarial streaming: Malicious XSS chunks inside code block", () => {
  const stream = [
    "```html\n",
    "<script>",
    "alert('xss')",
    "</script>\n",
    "<img src=x onerror=alert(1)>\n",
    "```",
  ];

  let accumulated = "";
  for (const chunk of stream) {
    accumulated += chunk;
    const rendered = renderSafeMarkdown(accumulated);
    validateHtmlWellFormedness(rendered);
    assert(!rendered.includes("<script>"), "Must never contain unescaped <script>");
    assert(!rendered.includes("<img "), "Must never contain unescaped <img>");
  }

  const finalRendered = renderSafeMarkdown(accumulated);
  assert(finalRendered.includes("&lt;script&gt;alert(&#39;xss&#39;)&lt;/script&gt;"), "Code block content escaped");
  assert(finalRendered.includes("&lt;img src=x onerror=alert(1)&gt;"), "Img tag escaped");
});

test("1.8 Adversarial streaming: Rapidly alternating odd backticks and asterisks", () => {
  const inputs = [
    "```",
    "``",
    "`",
    "`*",
    "`**",
    "`**`",
    "**`**`",
    "````code````",
    "***bold and italic?***",
  ];

  for (const inp of inputs) {
    const rendered = renderSafeMarkdown(inp);
    validateHtmlWellFormedness(rendered);
  }
});

// -----------------------------------------------------------------------------
// Suite 2: Truncation & Capsule Behavior
// -----------------------------------------------------------------------------
console.log("\n--- Suite 2: Truncation & Capsule Behavior ---");

test("2.1 Truncation at boundary: 2,999 characters (not truncated)", () => {
  const input = "A".repeat(2999);
  const result = truncateSelection(input, 3000);

  assertEqual(result.truncated, false, "truncated flag must be false");
  assertEqual(result.originalLen, 2999, "originalLen must be 2999");
  assertEqual(result.text.length, 2999, "text length must be 2999");
});

test("2.2 Truncation at boundary: exactly 3,000 characters (not truncated)", () => {
  const input = "B".repeat(3000);
  const result = truncateSelection(input, 3000);

  assertEqual(result.truncated, false, "truncated flag must be false");
  assertEqual(result.originalLen, 3000, "originalLen must be 3000");
  assertEqual(result.text.length, 3000, "text length must be 3000");
});

test("2.3 Truncation at boundary: 3,001 characters (truncated to 3,000)", () => {
  const input = "C".repeat(3001);
  const result = truncateSelection(input, 3000);

  assertEqual(result.truncated, true, "truncated flag must be true");
  assertEqual(result.originalLen, 3001, "originalLen must be 3001");
  assertEqual(result.text.length, 3000, "text length must be exactly 3000");
  assertEqual(result.text, "C".repeat(3000), "text content must match first 3000 chars");
});

test("2.4 Truncation with massive input: 15,000 characters (truncated to 3,000)", () => {
  const input = "D".repeat(15000);
  const result = truncateSelection(input, 3000);

  assertEqual(result.truncated, true, "truncated flag must be true");
  assertEqual(result.originalLen, 15000, "originalLen must be 15000");
  assertEqual(result.text.length, 3000, "text length must be exactly 3000");
});

test("2.5 Unicode surrogate pairs & multi-byte character safety (3005 code points)", () => {
  // 2998 Chinese chars + 3 surrogate emojis = 3001 code points
  const chinese = "字".repeat(2998);
  const emojis = "🎉🚀🔥"; // 3 emojis
  const extra = "尾巴"; // 2 chars -> total 3003 code points
  const input = chinese + emojis + extra;

  const codePointsCount = Array.from(input).length;
  assertEqual(codePointsCount, 3003, "Total code points must be 3003");

  const result = truncateSelection(input, 3000);
  assertEqual(result.truncated, true, "Must be truncated");
  assertEqual(result.originalLen, 3003, "originalLen matches code point count");

  const outCodePoints = Array.from(result.text);
  assertEqual(outCodePoints.length, 3000, "Truncated code point length must be 3000");

  // Verify that surrogate pairs were not bisected
  assert(!/[\uD800-\uDBFF]$/.test(result.text), "Must not end with a high surrogate");
  assert(!/^[\uDC00-\uDFFF]/.test(result.text), "Must not start with a low surrogate");
});

test("2.6 Capsule UI display text & conversation history truncation contract", () => {
  // Simulate the exact state machine from src/main.ts
  const mockUI = {
    capsuleDisplay: "none",
    capsuleText: "",
    conversationMessages: [],
  };

  function simulateStartNewQuery(text) {
    const truncatedResult = truncateSelection(text, 3000);

    // updateTruncationUI simulation
    if (truncatedResult.truncated) {
      mockUI.capsuleDisplay = "inline-flex";
      mockUI.capsuleText = `已截取前 3,000 字符（原文共 ${truncatedResult.originalLen.toLocaleString()} 字）· 保护上下文`;
    } else {
      mockUI.capsuleDisplay = "none";
      mockUI.capsuleText = "";
    }

    mockUI.conversationMessages = [{ role: "user", content: truncatedResult.text }];
  }

  // 1. Feed > 3000 chars (e.g. 4500 chars)
  const longText = "X".repeat(4500);
  simulateStartNewQuery(longText);

  assertEqual(mockUI.capsuleDisplay, "inline-flex", "Capsule must be visible (inline-flex)");
  assert(mockUI.capsuleText.includes("已截取前 3,000 字符"), "Capsule text includes prefix");
  assert(mockUI.capsuleText.includes("4,500"), "Capsule text includes formatted count 4,500");
  assertEqual(mockUI.conversationMessages[0].content.length, 3000, "Conversation history must be truncated to 3000");

  // 2. Feed <= 3000 chars (e.g. 500 chars)
  const shortText = "Y".repeat(500);
  simulateStartNewQuery(shortText);

  assertEqual(mockUI.capsuleDisplay, "none", "Capsule must be hidden for short text");
  assertEqual(mockUI.conversationMessages[0].content.length, 500, "Conversation history contains full 500 chars");
});

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
console.log("\n================================================================");
console.log(`  RESULTS: ${passedTests} / ${totalTests} passed, ${failedTests} failed.`);
console.log("================================================================");

if (failedTests > 0) {
  console.error("Failures:", failures);
  process.exit(1);
} else {
  console.log("All adversarial streaming & truncation challenges passed cleanly!");
}
