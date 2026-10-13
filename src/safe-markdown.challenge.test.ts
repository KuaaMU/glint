/**
 * Empirical Adversarial Challenge & Stress Test Suite
 * Milestone 1: R1 Safe Markdown & Unicode Truncation Defense
 *
 * Executed independently by teamwork_preview_challenger_m1_1.
 */

import {
  renderSafeMarkdown,
  truncateSelection,
} from "./safe-markdown.ts";

declare const process: { exit: (code?: number) => void };

let totalTests = 0;
let passedTests = 0;
let failedTests = 0;

function assert(condition: boolean, msg: string) {
  if (!condition) {
    throw new Error(`Assertion failed: ${msg}`);
  }
}

function runChallengeTest(name: string, fn: () => void) {
  totalTests++;
  try {
    fn();
    console.log(`[PASS] ${name}`);
    passedTests++;
  } catch (err: unknown) {
    console.error(`[FAIL] ${name}`);
    console.error(err);
    failedTests++;
  }
}

// Allowed HTML tags in sanitized markdown output
const ALLOWED_TAGS = new Set([
  "pre", "/pre",
  "code", "/code",
  "strong", "/strong",
  "em", "/em",
  "a", "/a",
  "ul", "/ul",
  "ol", "/ol",
  "li", "/li",
  "h1", "/h1",
  "h2", "/h2",
  "h3", "/h3",
  "h4", "/h4",
  "blockquote", "/blockquote",
  "hr", "/hr", "hr/",
  "br", "br/"
]);

/**
 * Parses tag attributes into key-value pairs following HTML5 attribute syntax
 */
function parseAttributes(attrString: string): Array<{ name: string; value: string }> {
  const clean = attrString.replace(/\/$/, "").trim();
  if (!clean) return [];

  const attrs: Array<{ name: string; value: string }> = [];
  const regex = /([a-zA-Z0-9_\.:-]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g;
  let match: RegExpExecArray | null;

  while ((match = regex.exec(clean)) !== null) {
    attrs.push({
      name: match[1].toLowerCase(),
      value: match[2] ?? match[3] ?? match[4] ?? "",
    });
  }
  return attrs;
}

function verifyHtmlSafety(html: string, testLabel: string) {
  // 1. Tag name audit
  const tagRegex = /<\s*(\/?[a-zA-Z0-9\-]+)([^>]*)>/g;
  let match: RegExpExecArray | null;

  while ((match = tagRegex.exec(html)) !== null) {
    const rawTag = match[1].toLowerCase();
    const rawAttrs = match[2];

    assert(
      ALLOWED_TAGS.has(rawTag),
      `[${testLabel}] Disallowed HTML tag detected: <${rawTag}> in output: ${html}`
    );

    const parsedAttrs = parseAttributes(rawAttrs);

    // 2. Event handler attribute audit: no attribute name may be an event handler (on*)
    for (const attr of parsedAttrs) {
      assert(
        !attr.name.startsWith("on"),
        `[${testLabel}] Event handler attribute detected: ${attr.name}="${attr.value}" in <${rawTag}>`
      );
      assert(
        !/^javascript\s*:/i.test(attr.value),
        `[${testLabel}] javascript: URI detected in attribute ${attr.name}="${attr.value}"`
      );
    }

    // 3. Strict schema validation for allowed tags
    if (rawTag === "pre") {
      assert(
        parsedAttrs.length === 1 && parsedAttrs[0].name === "class" && parsedAttrs[0].value === "glint-code-block",
        `[${testLabel}] Invalid <pre> attributes: ${JSON.stringify(parsedAttrs)}`
      );
    } else if (rawTag === "code") {
      assert(
        parsedAttrs.length === 1 &&
          ((parsedAttrs[0].name === "class" && parsedAttrs[0].value === "glint-inline-code") ||
            parsedAttrs[0].name === "data-lang"),
        `[${testLabel}] Unexpected <code> attributes: ${JSON.stringify(parsedAttrs)}`
      );
    } else if (rawTag === "strong" || rawTag === "br" || rawTag === "br/") {
      assert(
        parsedAttrs.length === 0,
        `[${testLabel}] <${rawTag}> must not have attributes, got: ${JSON.stringify(parsedAttrs)}`
      );
    }
  }

  // 4. Absolute prohibitions against raw executable tags
  assert(!/<script\b/i.test(html), `[${testLabel}] Unescaped <script> tag found!`);
  assert(!/<img\b/i.test(html), `[${testLabel}] Unescaped <img> tag found!`);
  assert(!/<svg\b/i.test(html), `[${testLabel}] Unescaped <svg> tag found!`);
  assert(!/<iframe\b/i.test(html), `[${testLabel}] Unescaped <iframe> tag found!`);
  assert(!/<a\b/i.test(html), `[${testLabel}] Unescaped <a> tag found!`);
}

console.log("=== STARTING ADVERSARIAL CHALLENGE SUITE (M1) ===\n");

// ============================================================================
// SECTION 1: XSS PENETRATION TESTING & INJECTION VECTORS
// ============================================================================

const XSS_VECTORS = [
  { name: "Classic script tag", input: "<script>alert('xss')</script>" },
  { name: "Script tag with attributes", input: '<script type="text/javascript" src="https://evil.com/x.js"></script>' },
  { name: "Case variation script", input: "<sCrIpt>alert(1)</sCrIpt>" },
  { name: "Image tag with onerror", input: "<img src=x onerror=alert(1)>" },
  { name: "Image tag with quoted onerror", input: '<img src=x onerror="alert(1)">' },
  { name: "Image tag with single-quoted onerror", input: "<img src=x onerror='alert(1)'>" },
  { name: "Image tag without src", input: "<img onerror=alert(1)>" },
  { name: "Image tag uppercase", input: '<IMG SRC="javascript:alert(\'XSS\');">' },
  { name: "SVG tag with onload", input: "<svg onload=alert(1)>" },
  { name: "SVG slash onload", input: "<svg/onload=alert(1)>" },
  { name: "SVG with embedded script", input: "<svg><script>alert(1)</script></svg>" },
  { name: "Iframe with javascript uri", input: '<iframe src="javascript:alert(1)"></iframe>' },
  { name: "Iframe unquoted javascript uri", input: "<iframe src=javascript:alert(1)>" },
  { name: "Anchor with javascript href", input: '<a href="javascript:alert(1)">Click me</a>' },
  { name: "Anchor with single quotes", input: "<a href='javascript:alert(1)'>Click me</a>" },
  { name: "Body onload injection", input: "<body onload=alert(1)>" },
  { name: "Input autofocus onfocus", input: '<input autofocus onfocus="alert(1)">' },
  { name: "Select autofocus onfocus", input: "<select autofocus onfocus=alert(1)>" },
  { name: "Textarea autofocus onfocus", input: "<textarea autofocus onfocus=alert(1)>" },
  { name: "Object with data URI", input: '<object data="data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==">' },
  { name: "Embed tag with javascript", input: '<embed src="javascript:alert(1)">' },
  { name: "Base href hijacking", input: '<base href="javascript:alert(1)//">' },
  { name: "Link stylesheet with javascript", input: '<link rel="stylesheet" href="javascript:alert(1)">' },
  { name: "Style tag with expression", input: "<style>body{background:expression(alert(1))}</style>" },
  { name: "Div with inline style event", input: '<div style="background-image: url(javascript:alert(1))">test</div>' },
  { name: "Malformed opening bracket", input: "><script>alert(1)</script>" },
  { name: "Unclosed image tag", input: "<img src=x onerror=alert(1)" },
  { name: "Unclosed script tag", input: "<script src=https://evil.com" },
  { name: "Whitespace separated svg", input: "<svg \t\n onload=alert(1)>" },
  { name: "Null byte injection attempt", input: "<scr\x00ipt>alert(1)</script>" },
  { name: "Double decode attempt 1", input: "&lt;script&gt;alert(1)&lt;/script&gt;" },
  { name: "Double decode attempt 2", input: "&amp;lt;script&gt;alert(1)&amp;lt;/script&gt;" },
  { name: "Numeric entity attempt", input: "&#60;script&#62;alert(1)&#60;/script&#62;" },
  { name: "Quote breakout attempt", input: '"> <script>alert(1)</script>' },
];

for (const vec of XSS_VECTORS) {
  runChallengeTest(`XSS Vector: ${vec.name}`, () => {
    const rendered = renderSafeMarkdown(vec.input);
    verifyHtmlSafety(rendered, vec.name);
  });
}

// ============================================================================
// SECTION 2: MARKDOWN & INLINE SYNTAX EDGE CASES
// ============================================================================

runChallengeTest("Markdown inside HTML tag attributes", () => {
  const input = '<div title="**bold text**">content</div>';
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Markdown inside HTML tag");
  assert(
    rendered.includes("&lt;div title=&quot;<strong>bold text</strong>&quot;&gt;content&lt;/div&gt;"),
    `Expected safe text entity wrapping, got: ${rendered}`
  );
});

runChallengeTest("Code fence language injection breakout attempt", () => {
  const input = '```html" onclick="alert(1)"\nconst a = 1;\n```';
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Code fence lang breakout");
  assert(
    rendered.includes('data-lang="html&quot; onclick=&quot;alert(1)&quot;"'),
    `data-lang must have escaped quotes: ${rendered}`
  );
  assert(!rendered.includes('onclick="alert(1)"'), "Must not have active onclick attribute");
});

runChallengeTest("Backticks inside code block are preserved safely", () => {
  const input = "```javascript\nconst msg = `User: ${name}`;\nconsole.log(msg);\n```";
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Backticks in code block");
  assert(
    rendered.includes("const msg = `User: ${name}`;"),
    `Inner backticks should be preserved verbatim: ${rendered}`
  );
});

runChallengeTest("Multiple backticks in inline code", () => {
  const input = "Use ``inline with ` single backtick`` here.";
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Multiple backticks inline");
});

runChallengeTest("Nested backtick attack vector", () => {
  const input = "`<script>`alert(1)`</script>`";
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Nested backtick attack");
  assert(
    !rendered.includes("<script>"),
    `Must not produce naked script tag: ${rendered}`
  );
});

runChallengeTest("Markdown inside script tags", () => {
  const input = "<script>**alert(1)**</script>";
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Markdown inside script");
  assert(
    rendered.includes("&lt;script&gt;<strong>alert(1)</strong>&lt;/script&gt;"),
    `Expected escaped script tag with bold: ${rendered}`
  );
});

runChallengeTest("Streaming unclosed code block with HTML payloads", () => {
  const input = "```typescript\n<img src=x onerror=alert(1)>\nconst x = '<script>evil()</script>';";
  const rendered = renderSafeMarkdown(input);
  verifyHtmlSafety(rendered, "Streaming unclosed code block");
  assert(
    rendered.includes("&lt;img src=x onerror=alert(1)&gt;"),
    `Payload inside unclosed code block must be escaped: ${rendered}`
  );
  assert(
    rendered.endsWith("</code></pre>"),
    `Unclosed code block must terminate with closing tags: ${rendered}`
  );
});

runChallengeTest("Fenced code blocks with 10,000 lines", () => {
  const lines = Array.from({ length: 10000 }, (_, i) => `let v${i} = ${i};`);
  const input = "```js\n" + lines.join("\n") + "\n```";
  const start = Date.now();
  const rendered = renderSafeMarkdown(input);
  const duration = Date.now() - start;
  verifyHtmlSafety(rendered, "10k lines code block");
  assert(duration < 2000, `Execution too slow: ${duration}ms`);
});

// ============================================================================
// SECTION 3: BOUNDARY & UNICODE TESTING (truncateSelection)
// ============================================================================

runChallengeTest("truncateSelection boundary: 0 characters", () => {
  const res = truncateSelection("");
  assert(res.text === "", "text must be empty");
  assert(res.truncated === false, "truncated must be false");
  assert(res.originalLen === 0, "originalLen must be 0");
});

runChallengeTest("truncateSelection boundary: 1 character", () => {
  const res = truncateSelection("X");
  assert(res.text === "X", "text must be X");
  assert(res.truncated === false, "truncated must be false");
  assert(res.originalLen === 1, "originalLen must be 1");
});

runChallengeTest("truncateSelection boundary: 2999 characters", () => {
  const str = "A".repeat(2999);
  const res = truncateSelection(str, 3000);
  assert(res.text.length === 2999, "length must be 2999");
  assert(res.truncated === false, "truncated must be false");
  assert(res.originalLen === 2999, "originalLen must be 2999");
});

runChallengeTest("truncateSelection boundary: 3000 characters", () => {
  const str = "B".repeat(3000);
  const res = truncateSelection(str, 3000);
  assert(res.text.length === 3000, "length must be 3000");
  assert(res.truncated === false, "truncated must be false");
  assert(res.originalLen === 3000, "originalLen must be 3000");
});

runChallengeTest("truncateSelection boundary: 3001 characters", () => {
  const str = "C".repeat(3001);
  const res = truncateSelection(str, 3000);
  assert(res.text.length === 3000, "length must be truncated to 3000");
  assert(res.text === "C".repeat(3000), "text must match prefix");
  assert(res.truncated === true, "truncated must be true");
  assert(res.originalLen === 3001, "originalLen must be 3001");
});

runChallengeTest("truncateSelection boundary: 5000 characters", () => {
  const str = "D".repeat(5000);
  const res = truncateSelection(str, 3000);
  assert(res.text.length === 3000, "length must be truncated to 3000");
  assert(res.text === "D".repeat(3000), "text must match prefix");
  assert(res.truncated === true, "truncated must be true");
  assert(res.originalLen === 5000, "originalLen must be 5000");
});

runChallengeTest("truncateSelection boundary: 50000 characters", () => {
  const str = "E".repeat(50000);
  const start = Date.now();
  const res = truncateSelection(str, 3000);
  const duration = Date.now() - start;
  assert(res.text.length === 3000, "length must be truncated to 3000");
  assert(res.text === "E".repeat(3000), "text must match prefix");
  assert(res.truncated === true, "truncated must be true");
  assert(res.originalLen === 50000, "originalLen must be 50000");
  assert(duration < 500, `Performance too slow for 50k chars: ${duration}ms`);
});

runChallengeTest("truncateSelection: Complex multi-codepoint emojis (👨‍👩‍👦, 🏳️‍🌈)", () => {
  const family = "👨‍👩‍👦";
  const flag = "🏳️‍🌈";
  const skinTone = "👍🏽";

  const input = `${family} - ${flag} - ${skinTone}`;
  const res = truncateSelection(input, 3000);

  assert(res.truncated === false, "must not truncate short string");
  assert(res.text === input, "text must be identical");
  assert(typeof (res.text as any).isWellFormed === "function" ? (res.text as any).isWellFormed() : true, "Must be well-formed UTF-16");
});

runChallengeTest("truncateSelection: Surrogate pair right at the 3000 boundary (2999 + 1 + 1)", () => {
  const prefix = "X".repeat(2999);
  const input = prefix + "🚀" + "!";
  const res = truncateSelection(input, 3000);

  assert(res.originalLen === 3001, "originalLen must be 3001");
  assert(res.truncated === true, "truncated must be true");

  const codepoints = Array.from(res.text);
  assert(codepoints.length === 3000, "codepoints length must be 3000");
  assert(codepoints[2999] === "🚀", "Rocket emoji must be preserved intact without splitting surrogates");
  assert(res.text.endsWith("🚀"), "res.text must end with intact rocket emoji");
  assert(typeof (res.text as any).isWellFormed === "function" ? (res.text as any).isWellFormed() : true, "Must be well-formed UTF-16");
});

runChallengeTest("truncateSelection: Exactly 3000 emojis (surrogate pairs)", () => {
  const emojis = "✨".repeat(3000);
  const res = truncateSelection(emojis, 3000);

  assert(res.originalLen === 3000, "originalLen must be 3000");
  assert(res.truncated === false, "truncated must be false");
  assert(res.text === emojis, "text must match exactly");
  const codepoints = Array.from(res.text);
  assert(codepoints.length === 3000, "codepoints length must be 3000");
});

runChallengeTest("truncateSelection: CJK Text and CJK Extension B (SIP surrogate pairs)", () => {
  const cjkCommon = "透视模型架构与原生渲染系统。";
  const cjkRare = "𠀀𠀁𠀂𠀃";
  const input = cjkCommon + cjkRare;
  const res = truncateSelection(input, 3000);

  assert(res.truncated === false, "not truncated");
  assert(res.text === input, "text intact");
  assert(typeof (res.text as any).isWellFormed === "function" ? (res.text as any).isWellFormed() : true, "Must be well-formed UTF-16");
});

runChallengeTest("truncateSelection: Combining accents and diacritics", () => {
  const accented = "e\u0301".repeat(1500);
  const res = truncateSelection(accented, 3000);
  assert(res.originalLen === 3000, "originalLen must be 3000");
  assert(res.truncated === false, "truncated must be false");

  const accentedOver = "e\u0301".repeat(1501);
  const resOver = truncateSelection(accentedOver, 3000);
  assert(resOver.truncated === true, "must be truncated");
  assert(Array.from(resOver.text).length === 3000, "must have 3000 codepoints");
  assert(typeof (resOver.text as any).isWellFormed === "function" ? (resOver.text as any).isWellFormed() : true, "Must be well-formed UTF-16");
});

// ============================================================================
// SUMMARY & VERDICT
// ============================================================================

console.log("\n========================================");
console.log(`Adversarial Tests Executed: ${totalTests}`);
console.log(`Passed: ${passedTests}`);
console.log(`Failed: ${failedTests}`);
console.log("========================================\n");

if (failedTests > 0) {
  console.error("ADVERSARIAL CHALLENGE FAILED: BUGS DETECTED.");
  process.exit(1);
} else {
  console.log("ALL ADVERSARIAL CHALLENGE TESTS PASSED CLEANLY!");
}
