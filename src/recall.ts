/**
 * Echo helpers: the pure half of Glint's local recall.
 *
 * Nothing here touches Tauri, the DOM, or storage. `main.ts` owns the I/O, so
 * the rules that decide *what the user sees and what the model is told* can be
 * tested in a plain Node process.
 */

import type { ChatMessage } from "./history-store.ts";

/** A prior answer about the same concept, from `glint-recall`. */
export interface RecallEcho {
  turn_id: number;
  conclusion: string;
  app_class: string;
  age_ms: number;
  thread_id: string;
  matched_anchor: string;
}

export function formatEchoAge(ageMs: number): string {
  const minutes = Math.floor(ageMs / 60_000);
  if (minutes < 60) return "刚才";
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  const months = Math.floor(days / 30);
  return months < 12 ? `${months} 个月前` : `${Math.floor(months / 12)} 年前`;
}

export function echoAppLabel(appClass: string): string {
  const key = appClass.trim().toLowerCase();
  if (!key) return "";
  if (key.includes("code") || key.includes("vscode")) return "VS Code";
  if (key.includes("claude")) return "Claude";
  if (key.includes("firefox")) return "Firefox";
  if (key.includes("chrome") || key.includes("chromium")) return "Chrome";
  if (key.includes("edge")) return "Edge";
  if (key.includes("konsole") || key.includes("terminal")) return "终端";
  if (key.includes("okular")) return "Okular";

  // Linux app classes are often reverse-DNS (`com.anthropic.claude`). The last
  // segment is the part a person recognises.
  const last = key.includes(".") ? (key.split(".").pop() ?? "") : key;
  return last ? last.charAt(0).toUpperCase() + last.slice(1) : "";
}

/** `在 VS Code ` when there is a source, nothing when there is not. */
function echoWhere(echo: RecallEcho): string {
  const app = echoAppLabel(echo.app_class);
  return app ? `在 ${app} ` : "";
}

/**
 * A stored conclusion is model output derived from untrusted documents. Strip
 * the data-block delimiters and invisible controls before it goes back into a
 * prompt, otherwise a prior answer can close `<prior_answer>` early.
 */
export function sanitizeEchoText(text: string): string {
  return text
    .replace(/<\/?prior_answer>/gi, "")
    .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u202A-\u202E\u2066-\u2069]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

/** The single HUD line. A miss never calls this: there is no empty state. */
export function echoLineText(echo: RecallEcho): string {
  const conclusion = sanitizeEchoText(echo.conclusion);
  return `↺ ${formatEchoAge(echo.age_ms)}${echoWhere(echo)}问过 · ${conclusion}`;
}

/**
 * Attach the prior answer to the current user turn for this request only.
 * The conversation array itself stays clean: the block must not appear in the
 * card or in the persisted history.
 */
export function withPriorAnswer(messages: ChatMessage[], echo: RecallEcho): ChatMessage[] {
  if (messages.length === 0) return messages;

  const conclusion = sanitizeEchoText(echo.conclusion);
  if (!conclusion) return messages;

  const block =
    `<prior_answer>\n` +
    `这是该用户 ${formatEchoAge(echo.age_ms)}${echoWhere(echo)}问过的同一概念，当时的结论：${conclusion}\n` +
    `这是你此前的回答，可能不完整或有误；与当前证据冲突时以当前证据为准，并直接说出差异。与当前问题无关时不要提及。\n` +
    `</prior_answer>\n\n`;

  const copy = messages.slice();
  const last = copy[copy.length - 1];

  if (typeof last.content === "string") {
    copy[copy.length - 1] = { role: last.role, content: block + last.content };
  } else {
    const parts = last.content.slice();
    const firstText = parts.findIndex((part) => part.type === "text");
    if (firstText >= 0) {
      const part = parts[firstText] as { type: "text"; text: string };
      parts[firstText] = { type: "text", text: block + part.text };
    } else {
      parts.unshift({ type: "text", text: block });
    }
    copy[copy.length - 1] = { role: last.role, content: parts };
  }

  return copy;
}

/** The first sentence is the conclusion; the rest stays in the old card. */
export function conclusionFrom(answer: string): string {
  const clean = answer
    .replace(/```[\s\S]*?```/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  if (!clean) return "";

  // Chinese sentence marks need no trailing space.
  const cjk = clean.match(/^(.{1,120}?[。！？])/);
  if (cjk) return cjk[1].trim();

  // A Latin period ends a sentence only when whitespace or the end follows;
  // otherwise it is an abbreviation or a version number.
  const latin = clean.match(/^(.{1,120}?[.!?])(?:\s|$)/);
  return (latin ? latin[1] : clean.slice(0, 120)).trim();
}
