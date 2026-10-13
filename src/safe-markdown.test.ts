/**
 * Comprehensive Unit Test Suite for Glint Safe Markdown & Truncation Pipeline
 *
 * Verifies:
 * - HTML escaping & zero-XSS execution across attack vectors
 * - Markdown parsing (fenced code, streaming unclosed code, inline code, bold, newlines)
 * - Unicode-aware truncation at 0, 2999, 3000, 3001, 10000 characters and emoji boundaries
 */

import {
  escapeHtml,
  parseInlineTokens,
  renderSafeMarkdown,
  truncateSelection,
} from "./safe-markdown.ts";

declare const process: { exit: (code?: number) => void };

const assert = {
  strictEqual<T>(actual: T, expected: T, message?: string): void {
    if (actual !== expected) {
      throw new Error(message || `Expected: ${JSON.stringify(expected)}, got: ${JSON.stringify(actual)}`);
    }
  },
  ok(condition: unknown, message?: string): void {
    if (!condition) {
      throw new Error(message || "Expected truthy condition");
    }
  },
};

let passCount = 0;
let failCount = 0;

function runTest(name: string, fn: () => void) {
  try {
    fn();
    console.log(`✓ PASS: ${name}`);
    passCount++;
  } catch (err: unknown) {
    console.error(`✗ FAIL: ${name}`);
    console.error(err);
    failCount++;
  }
}

// -----------------------------------------------------------------------------
// 1. HTML Escaping Tests
// -----------------------------------------------------------------------------
runTest("escapeHtml escapes all dangerous HTML entities (&, <, >, \", ')", () => {
  const raw = `<script>alert("XSS & 'injection'")</script>`;
  const expected = `&lt;script&gt;alert(&quot;XSS &amp; &#39;injection&#39;&quot;)&lt;/script&gt;`;
  assert.strictEqual(escapeHtml(raw), expected);
});

runTest("escapeHtml handles empty or null-like strings", () => {
  assert.strictEqual(escapeHtml(""), "");
});

// -----------------------------------------------------------------------------
// 2. Inline Token Parsing Direct Tests
// -----------------------------------------------------------------------------
runTest("parseInlineTokens formats inline code and bold", () => {
  const input = "Here is `my_code` and **my_bold**.";
  const rendered = parseInlineTokens(input);
  assert.strictEqual(
    rendered,
    'Here is <code class="glint-inline-code">my_code</code> and <strong>my_bold</strong>.'
  );
});

runTest("parseInlineTokens handles unclosed backtick and asterisks gracefully", () => {
  const input = "Unclosed `code and **unclosed bold";
  const rendered = parseInlineTokens(input);
  assert.strictEqual(rendered, "Unclosed `code and **unclosed bold");
});

// -----------------------------------------------------------------------------
// 3. XSS Injection Vector Neutralization Tests
// -----------------------------------------------------------------------------
runTest("renderSafeMarkdown neutralizes <script> injection", () => {
  const payload = "<script>alert('pwned')</script>";
  const rendered = renderSafeMarkdown(payload);
  assert.ok(!rendered.includes("<script>"), "Must not contain unescaped <script>");
  assert.ok(rendered.includes("&lt;script&gt;alert(&#39;pwned&#39;)&lt;/script&gt;"));
});

runTest("renderSafeMarkdown neutralizes <img src=x onerror=...> injection", () => {
  const payload = '<img src=x onerror="alert(1)">';
  const rendered = renderSafeMarkdown(payload);
  assert.ok(!rendered.includes("<img "), "Must not contain unescaped <img>");
  assert.ok(rendered.includes("&lt;img src=x onerror=&quot;alert(1)&quot;&gt;"));
});

runTest("renderSafeMarkdown neutralizes <svg/onload=...> injection", () => {
  const payload = "<svg/onload=alert(1)>";
  const rendered = renderSafeMarkdown(payload);
  assert.ok(!rendered.includes("<svg"), "Must not contain unescaped <svg>");
  assert.ok(rendered.includes("&lt;svg/onload=alert(1)&gt;"));
});

runTest("renderSafeMarkdown neutralizes <iframe>, <style>, <body onload=...>", () => {
  const payload = '<iframe src="javascript:alert(1)"></iframe><body onload=evil()>';
  const rendered = renderSafeMarkdown(payload);
  assert.ok(!rendered.includes("<iframe"), "Must not contain unescaped <iframe>");
  assert.ok(!rendered.includes("<body"), "Must not contain unescaped <body>");
  assert.ok(rendered.includes("&lt;iframe"));
  assert.ok(rendered.includes("&lt;body"));
});

// -----------------------------------------------------------------------------
// 4. Markdown Formatting Tests
// -----------------------------------------------------------------------------
runTest("renderSafeMarkdown parses fenced code blocks with language and escapes inner code", () => {
  const input = "Here is TypeScript:\n```typescript\nfunction greet(name: string) {\n  return `<script>${name}</script>`;\n}\n```\nEnd of code.";
  const rendered = renderSafeMarkdown(input);

  assert.ok(
    rendered.includes('<pre class="glint-code-block"><code data-lang="typescript">'),
    `Expected code block tag, got: ${rendered}`
  );
  assert.ok(!rendered.includes("<script>"), "Code inside code block must be escaped");
  assert.ok(rendered.includes("&lt;script&gt;"));
  assert.ok(rendered.includes("</code></pre>"));
  assert.ok(rendered.includes("End of code."));
});

runTest("renderSafeMarkdown parses fenced code blocks without language", () => {
  const input = "```\nplain text code\n```";
  const rendered = renderSafeMarkdown(input);

  assert.ok(
    rendered.includes('<pre class="glint-code-block"><code data-lang="">plain text code</code></pre>'),
    `Expected plain code block, got: ${rendered}`
  );
});

runTest("renderSafeMarkdown handles streaming unclosed code blocks safely", () => {
  const streamingInput = "Streaming response:\n```rust\nfn main() {\n    let answer = 42;";
  const rendered = renderSafeMarkdown(streamingInput);

  assert.ok(
    rendered.includes('<pre class="glint-code-block"><code data-lang="rust">fn main() {\n    let answer = 42;</code></pre>'),
    `Expected properly closed pre/code tag, got: ${rendered}`
  );
  assert.ok(!rendered.endsWith("```"), "Must not leak unparsed fence");
});

runTest("renderSafeMarkdown parses inline code with .glint-inline-code class", () => {
  const input = "Please run `cargo test --all` in the terminal.";
  const rendered = renderSafeMarkdown(input);

  assert.strictEqual(
    rendered,
    'Please run <code class="glint-inline-code">cargo test --all</code> in the terminal.'
  );
});

runTest("renderSafeMarkdown parses inline code containing HTML safely", () => {
  const input = "Dangerous tag `<img onerror=alert(1)>` inside inline code.";
  const rendered = renderSafeMarkdown(input);

  assert.ok(!rendered.includes("<img "), "Must not have raw <img>");
  assert.strictEqual(
    rendered,
    'Dangerous tag <code class="glint-inline-code">&lt;img onerror=alert(1)&gt;</code> inside inline code.'
  );
});

runTest("renderSafeMarkdown parses bold text with <strong>", () => {
  const input = "This is **very important** note.";
  const rendered = renderSafeMarkdown(input);

  assert.strictEqual(rendered, "This is <strong>very important</strong> note.");
});

runTest("renderSafeMarkdown handles nested inline code inside bold text", () => {
  const input = "Important: **Use `npm run build` first** before shipping.";
  const rendered = renderSafeMarkdown(input);

  assert.strictEqual(
    rendered,
    'Important: <strong>Use <code class="glint-inline-code">npm run build</code> first</strong> before shipping.'
  );
});

runTest("renderSafeMarkdown converts single newlines to <br/> and double newlines to <br/><br/>", () => {
  const single = "Line 1\nLine 2";
  assert.strictEqual(renderSafeMarkdown(single), "Line 1<br/>Line 2");

  const double = "Paragraph 1\n\nParagraph 2";
  assert.strictEqual(renderSafeMarkdown(double), "Paragraph 1<br/><br/>Paragraph 2");

  const crlf = "Windows 1\r\nWindows 2\r\n\r\nWindows 3";
  assert.strictEqual(renderSafeMarkdown(crlf), "Windows 1<br/>Windows 2<br/><br/>Windows 3");
});

// -----------------------------------------------------------------------------
// 5. Selection Truncation Boundary Tests
// -----------------------------------------------------------------------------
runTest("truncateSelection with empty string", () => {
  const res = truncateSelection("");
  assert.strictEqual(res.text, "");
  assert.strictEqual(res.truncated, false);
  assert.strictEqual(res.originalLen, 0);
});

runTest("truncateSelection boundary: 2999 characters (not truncated)", () => {
  const input = "a".repeat(2999);
  const res = truncateSelection(input, 3000);
  assert.strictEqual(res.text.length, 2999);
  assert.strictEqual(res.truncated, false);
  assert.strictEqual(res.originalLen, 2999);
});

runTest("truncateSelection boundary: exactly 3000 characters (not truncated)", () => {
  const input = "b".repeat(3000);
  const res = truncateSelection(input, 3000);
  assert.strictEqual(res.text.length, 3000);
  assert.strictEqual(res.truncated, false);
  assert.strictEqual(res.originalLen, 3000);
});

runTest("truncateSelection boundary: 3001 characters (truncated to 3000)", () => {
  const input = "c".repeat(3001);
  const res = truncateSelection(input, 3000);
  assert.strictEqual(res.text.length, 3000);
  assert.strictEqual(res.text, "c".repeat(3000));
  assert.strictEqual(res.truncated, true);
  assert.strictEqual(res.originalLen, 3001);
});

runTest("truncateSelection boundary: 10000 characters (truncated to 3000)", () => {
  const input = "d".repeat(10000);
  const res = truncateSelection(input, 3000);
  assert.strictEqual(res.text.length, 3000);
  assert.strictEqual(res.text, "d".repeat(3000));
  assert.strictEqual(res.truncated, true);
  assert.strictEqual(res.originalLen, 10000);
});

runTest("truncateSelection with Unicode surrogate pairs and emojis", () => {
  // 2999 emojis + 1 emoji + 1 standard char = 3001 Unicode characters
  const prefix = "🌟".repeat(2999);
  const input = prefix + "🚀" + "!";
  const res = truncateSelection(input, 3000);

  assert.strictEqual(res.originalLen, 3001);
  assert.strictEqual(res.truncated, true);
  // Exactly 2999 🌟 + 1 🚀 = 3000 code points, without broken surrogates
  const outChars = Array.from(res.text);
  assert.strictEqual(outChars.length, 3000);
  assert.strictEqual(outChars[2999], "🚀");
});

// -----------------------------------------------------------------------------
// Summary
// -----------------------------------------------------------------------------
console.log(`\n========================================`);
console.log(`Tests Passed: ${passCount}`);
console.log(`Tests Failed: ${failCount}`);
console.log(`========================================`);

if (failCount > 0) {
  process.exit(1);
} else {
  console.log("All safe-markdown unit tests passed cleanly!\n");
}
