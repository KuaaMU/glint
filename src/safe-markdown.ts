/**
 * Glint R1: Safe HTML Escaping and Lightweight Markdown Pipeline
 *
 * Zero external dependencies. Immune to XSS vectors.
 * Fenced code blocks, inline code, bold text, newlines, and Unicode-aware truncation.
 */

export interface TruncateResult {
  text: string;
  truncated: boolean;
  originalLen: number;
}

/**
 * Escapes HTML special entities to neutralize all script injection and HTML tag execution.
 * Order: & must be escaped first to prevent double-escaping &amp;.
 */
export function escapeHtml(raw: string): string {
  if (!raw) return "";
  return raw
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * Parses inline formatting (inline code `...`, bold **...**, italics *...*, links [text](url))
 * within pre-escaped text.
 */
export function parseInlineTokens(escapedText: string): string {
  if (!escapedText) return "";
  let result = "";
  let i = 0;
  const len = escapedText.length;

  while (i < len) {
    // 1. Inline code: `[code]`
    if (escapedText[i] === "`") {
      const closeIdx = escapedText.indexOf("`", i + 1);
      if (closeIdx !== -1) {
        const codeContent = escapedText.slice(i + 1, closeIdx);
        result += `<code class="glint-inline-code">${codeContent}</code>`;
        i = closeIdx + 1;
        continue;
      }
    }

    // 2. Bold text: **[text]**
    if (escapedText.startsWith("**", i)) {
      const closeIdx = escapedText.indexOf("**", i + 2);
      if (closeIdx !== -1 && closeIdx > i + 2) {
        const boldContent = escapedText.slice(i + 2, closeIdx);
        result += `<strong>${parseInlineTokens(boldContent)}</strong>`;
        i = closeIdx + 2;
        continue;
      }
    }

    // 3. Links: [text](https://...)
    if (escapedText[i] === "[") {
      const bracketClose = escapedText.indexOf("]", i + 1);
      if (bracketClose !== -1 && escapedText[bracketClose + 1] === "(") {
        const parenClose = escapedText.indexOf(")", bracketClose + 2);
        if (parenClose !== -1) {
          const linkText = escapedText.slice(i + 1, bracketClose);
          const rawUrl = escapedText.slice(bracketClose + 2, parenClose);
          // Only allow safe HTTP/HTTPS URLs
          const unescapedUrl = rawUrl.replace(/&amp;/g, "&");
          if (/^(https?:\/\/|\/|#)/i.test(unescapedUrl)) {
            result += `<a class="glint-link" href="${rawUrl}" target="_blank" rel="noopener noreferrer">${parseInlineTokens(linkText)}</a>`;
            i = parenClose + 1;
            continue;
          }
        }
      }
    }

    // 4. Italics text: *[text]* (must not be preceded or followed by *)
    if (escapedText[i] === "*" && escapedText[i + 1] !== "*" && (i === 0 || escapedText[i - 1] !== "*")) {
      const closeIdx = escapedText.indexOf("*", i + 1);
      if (closeIdx !== -1 && escapedText[closeIdx + 1] !== "*" && closeIdx > i + 1) {
        const italicContent = escapedText.slice(i + 1, closeIdx);
        result += `<em>${parseInlineTokens(italicContent)}</em>`;
        i = closeIdx + 1;
        continue;
      }
    }

    result += escapedText[i];
    i++;
  }

  return result;
}

/**
 * Safe markdown renderer for Glint streaming companion HUD.
 *
 * Transforms:
 * - Fenced code blocks ```[lang]\n[code]\n``` -> <pre class="glint-code-block"><code data-lang="...">...</code></pre>
 * - Streaming open code blocks gracefully without broken tags
 * - Inline code `[code]` -> <code class="glint-inline-code">...</code>
 * - Bold text **[text]** -> <strong>...</strong>
 * - \n\n -> <br/><br/> and \n -> <br/>
 */
import {
  renderCompareWidget,
  renderFlowWidget,
  renderMetricsWidget,
  renderCalloutWidget,
} from "./generative-ui.ts";

/**
 * Infer a widget from the fenced block's language tag OR from its JSON shape.
 *
 * Models frequently emit ```json (or no tag at all) for a widget payload. The
 * shape check keeps those payloads from degrading into a raw JSON code block:
 * `columns` + `rows` is a comparison matrix, `steps` a flow, `items` a metric
 * grid, and a lone `content` a callout.
 */
function inferWidgetType(lang: string, rawContent: string): string | null {
  const trimmedLang = lang.trim().toLowerCase();
  if (trimmedLang.startsWith("widget:")) {
    const declared = trimmedLang.slice("widget:".length).trim();
    if (declared === "timeline") return "flow";
    if (declared === "keyvals") return "metrics";
    return declared;
  }

  const looksLikeWidget =
    trimmedLang === "" || trimmedLang === "json" || trimmedLang === "widget";
  if (!looksLikeWidget) return null;

  let parsed: unknown;
  try {
    parsed = JSON.parse(rawContent);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    return null;
  }

  const record = parsed as Record<string, unknown>;
  if (Array.isArray(record.columns) && Array.isArray(record.rows)) return "compare";
  if (Array.isArray(record.steps)) return "flow";
  if (Array.isArray(record.items)) return "metrics";
  if (typeof record.content === "string" && typeof record.type === "string") {
    return "callout";
  }
  return null;
}

function renderWidgetOrCodeBlock(lang: string, lines: string[]): string {
  const trimmedLang = lang.trim();
  const rawContent = lines.join("\n");

  switch (inferWidgetType(trimmedLang, rawContent)) {
    case "compare":
      return renderCompareWidget(rawContent);
    case "flow":
      return renderFlowWidget(rawContent);
    case "metrics":
      return renderMetricsWidget(rawContent);
    case "callout":
      return renderCalloutWidget(rawContent);
  }

  const escapedCode = lines.map((l) => escapeHtml(l)).join("\n");
  const langAttr = trimmedLang ? ` data-lang="${escapeHtml(trimmedLang)}"` : ` data-lang=""`;
  return `<pre class="glint-code-block"><code${langAttr}>${escapedCode}</code></pre>`;
}

export function renderSafeMarkdown(markdown: string): string {
  if (!markdown) return "";

  // Normalize Windows/Mac line endings to \n
  const normalized = markdown.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const rawLines = normalized.split("\n");
  const processedBlocks: string[] = [];

  let inCodeBlock = false;
  let codeBlockLang = "";
  let codeBlockLines: string[] = [];

  // 状态机处理行级元素
  let currentListType: "ul" | "ol" | null = null;
  let listItems: string[] = [];

  const flushList = () => {
    if (!currentListType || listItems.length === 0) return;
    const tag = currentListType;
    const cls = currentListType === "ul" ? "glint-list-ul" : "glint-list-ol";
    const inner = listItems.map((it) => `<li class="glint-list-item">${it}</li>`).join("");
    processedBlocks.push(`<${tag} class="${cls}">${inner}</${tag}>`);
    currentListType = null;
    listItems = [];
  };

  for (let i = 0; i < rawLines.length; i++) {
    const line = rawLines[i];
    const trimmed = line.trim();

    // 围栏代码块检测
    if (trimmed.startsWith("```")) {
      flushList();
      if (inCodeBlock) {
        processedBlocks.push(renderWidgetOrCodeBlock(codeBlockLang, codeBlockLines));
        inCodeBlock = false;
        codeBlockLang = "";
        codeBlockLines = [];
      } else {
        inCodeBlock = true;
        codeBlockLang = trimmed.slice(3).trim();
        codeBlockLines = [];
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockLines.push(line);
      continue;
    }

    // 空行：重置列表
    if (trimmed === "") {
      flushList();
      processedBlocks.push("");
      continue;
    }

    // 1. 无序列表: - item 或 * item
    const ulMatch = line.match(/^(\s*)([-*+])\s+(.+)$/);
    if (ulMatch && ulMatch[2] !== "*" || (ulMatch && !trimmed.startsWith("**"))) {
      if (currentListType !== "ul") {
        flushList();
        currentListType = "ul";
      }
      const itemContent = parseInlineTokens(escapeHtml(ulMatch[3]));
      listItems.push(itemContent);
      continue;
    }

    // 2. 有序列表: 1. item
    const olMatch = line.match(/^(\s*)(\d+)\.\s+(.+)$/);
    if (olMatch) {
      if (currentListType !== "ol") {
        flushList();
        currentListType = "ol";
      }
      const itemContent = parseInlineTokens(escapeHtml(olMatch[3]));
      listItems.push(itemContent);
      continue;
    }

    // 非列表行，刷新正在收集的列表
    flushList();

    // 3. 标题: # H1, ## H2, ### H3, #### H4
    const headingMatch = trimmed.match(/^(#{1,4})\s+(.+)$/);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const headingText = parseInlineTokens(escapeHtml(headingMatch[2]));
      processedBlocks.push(`<h${level} class="glint-heading glint-h${level}">${headingText}</h${level}>`);
      continue;
    }

    // 4. 引用块: > quote
    if (trimmed.startsWith(">")) {
      const quoteText = parseInlineTokens(escapeHtml(trimmed.slice(1).trim()));
      processedBlocks.push(`<blockquote class="glint-blockquote">${quoteText}</blockquote>`);
      continue;
    }

    // 5. 分割线: --- or ***
    if (/^([-*_]){3,}$/.test(trimmed)) {
      processedBlocks.push(`<hr class="glint-hr" />`);
      continue;
    }

    // 普通文本行
    const escaped = escapeHtml(line);
    processedBlocks.push(parseInlineTokens(escaped));
  }

  // 收尾：流式未闭合代码块或未闭合列表
  flushList();
  if (inCodeBlock) {
    processedBlocks.push(renderWidgetOrCodeBlock(codeBlockLang, codeBlockLines));
  }

  // 拼接 HTML 块
  let html = "";
  const isBlockElement = (block: string): boolean =>
    block.startsWith('<pre class="glint-code-block"') ||
    block.startsWith('<ul class="') ||
    block.startsWith('<ol class="') ||
    block.startsWith('<h') ||
    block.startsWith('<blockquote') ||
    block.startsWith('<hr') ||
    block.includes('class="glint-widget');

  for (let i = 0; i < processedBlocks.length; i++) {
    const part = processedBlocks[i];
    if (part === "") {
      // A leading blank line is an artifact of splitting a string that starts
      // with "\n", not a visual line break. Emitting it would make
      // renderSafeMarkdown("\n") produce two <br/> instead of the contracted one.
      if (i > 0) html += "<br/>";
      continue;
    }

    const isBlock = isBlockElement(part);
    const prevIsBlock = i > 0 && isBlockElement(processedBlocks[i - 1]);

    if (i === 0 || isBlock || prevIsBlock) {
      html += part;
    } else {
      html += "<br/>" + part;
    }
  }

  return html;
}

/**
 * Unicode-aware selection truncation defense to prevent context window overflow.
 * Uses Array.from to safely preserve surrogate pairs, emojis, and multi-byte characters.
 */
export function truncateSelection(text: string, maxLen = 3000): TruncateResult {
  if (!text) {
    return { text: "", truncated: false, originalLen: 0 };
  }

  const chars = Array.from(text);
  const originalLen = chars.length;

  if (originalLen > maxLen) {
    return {
      text: chars.slice(0, maxLen).join(""),
      truncated: true,
      originalLen,
    };
  }

  return {
    text,
    truncated: false,
    originalLen,
  };
}

/**
 * Safely empties all child nodes of a DOM element without using innerHTML.
 */
export function emptyElement(el: HTMLElement): void {
  if (typeof el.replaceChildren === "function") {
    el.replaceChildren();
  } else {
    while (el.firstChild) {
      el.removeChild(el.firstChild);
    }
  }
}
