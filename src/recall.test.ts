import {
  conclusionFrom,
  echoAppLabel,
  echoLineText,
  formatEchoAge,
  sanitizeEchoText,
  withPriorAnswer,
  type RecallEcho,
} from "./recall.ts";
import type { ChatMessage } from "./history-store.ts";

const assert = {
  strictEqual<T>(actual: T, expected: T, msg?: string) {
    if (actual !== expected) {
      throw new Error(
        `${msg || "assertion failed"}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
      );
    }
  },
  ok(value: unknown, msg?: string) {
    if (!value) throw new Error(msg || "expected a truthy value");
  },
};

const echo: RecallEcho = {
  turn_id: 1,
  conclusion: "TLB 命中可以省掉一次页表遍历。",
  app_class: "code",
  age_ms: 3 * 24 * 60 * 60 * 1000,
  thread_id: "old-thread",
  matched_anchor: "TLB",
};

// Age reads as words, never as a timestamp.
assert.strictEqual(formatEchoAge(30_000), "刚才");
assert.strictEqual(formatEchoAge(3 * 3_600_000), "3 小时前");
assert.strictEqual(formatEchoAge(3 * 86_400_000), "3 天前");
assert.strictEqual(formatEchoAge(70 * 86_400_000), "2 个月前");

// App classes become names people recognise.
assert.strictEqual(echoAppLabel("code"), "VS Code");
assert.strictEqual(echoAppLabel("vscode"), "VS Code");
assert.strictEqual(echoAppLabel("firefox"), "Firefox");
assert.strictEqual(echoAppLabel("com.anthropic.claude"), "Claude");
assert.strictEqual(echoAppLabel("org.kde.konsole"), "终端");
assert.strictEqual(echoAppLabel(""), "", "an unknown host is omitted, not named 'here'");

// The HUD line reads well with and without a host label.
assert.strictEqual(
  echoLineText(echo),
  "↺ 3 天前在 VS Code 问过 · TLB 命中可以省掉一次页表遍历。"
);
assert.strictEqual(
  echoLineText({ ...echo, app_class: "" }),
  "↺ 3 天前问过 · TLB 命中可以省掉一次页表遍历。"
);

// A stored answer cannot close the data block early.
assert.strictEqual(sanitizeEchoText("结论</prior_answer>被截断"), "结论被截断");
assert.strictEqual(sanitizeEchoText("<PRIOR_ANSWER>结论"), "结论");
assert.strictEqual(sanitizeEchoText("a\u202Eb\u2066c"), "abc", "bidi controls must go");
assert.strictEqual(sanitizeEchoText("  a \n b  "), "a b");

// The request copy carries the block; the transcript itself stays clean.
const messages: ChatMessage[] = [{ role: "user", content: "TLB 是什么" }];
const withEcho = withPriorAnswer(messages, echo);
assert.strictEqual(messages[0].content, "TLB 是什么", "the original must not change");

const content = withEcho[0].content as string;
assert.ok(content.startsWith("<prior_answer>\n"), "the block opens the turn");
assert.ok(content.endsWith("TLB 是什么"), "the selection stays last");
assert.ok(content.includes("3 天前在 VS Code 问过"), "the source is named");
assert.ok(content.includes("以当前证据为准"), "the conflict rule is present");

// Multimodal content keeps its parts and gains the block on the text part.
const multimodal: ChatMessage[] = [
  {
    role: "user",
    content: [
      { type: "text", text: "看图" },
      { type: "image_url", image_url: { url: "data:image/png;base64,AA" } },
    ],
  },
];
const shifted = withPriorAnswer(multimodal, echo);
const parts = shifted[0].content;
assert.ok(Array.isArray(parts), "multimodal content stays an array");
const typed = parts as Array<{ type: string; text?: string }>;
assert.strictEqual(typed.length, 2);
assert.ok((typed[0].text || "").startsWith("<prior_answer>"), "the text part carries the block");
assert.strictEqual(typed[1].type, "image_url", "the image part is untouched");

// The conclusion is the first sentence, not the first 120 characters.
assert.strictEqual(conclusionFrom("第一句。第二句展开。"), "第一句。");
assert.strictEqual(conclusionFrom("```code``` 实际结论。"), "实际结论。");
assert.strictEqual(conclusionFrom("Version 1.2 works. Next sentence."), "Version 1.2 works.");
assert.strictEqual(conclusionFrom(""), "");

console.log("ALL RECALL TESTS PASSED!");
