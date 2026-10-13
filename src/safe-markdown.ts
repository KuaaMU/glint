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
 * Parses inline formatting (inline code `...` and bold **...**) within pre-escaped text.
 * Content inside inline code is preserved as pre-escaped literals.
 * Bold text supports nested inline code.
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

function renderWidgetOrCodeBlock(lang: string, lines: string[]): string {
  const trimmedLang = lang.trim();
  const rawContent = lines.join("\n");

  if (trimmedLang.startsWith("widget:")) {
    const wType = trimmedLang.slice("widget:".length).trim().toLowerCase();
    switch (wType) {
      case "compare":
        return renderCompareWidget(rawContent);
      case "flow":
      case "timeline":
        return renderFlowWidget(rawContent);
      case "metrics":
      case "keyvals":
        return renderMetricsWidget(rawContent);
      case "callout":
        return renderCalloutWidget(rawContent);
    }
  }

  const escapedCode = lines.map((l) => escapeHtml(l)).join("\n");
  const langAttr = trimmedLang ? ` data-lang="${escapeHtml(trimmedLang)}"` : ` data-lang=""`;
  return `<pre class="glint-code-block"><code${langAttr}>${escapedCode}</code></pre>`;
}

export function renderSafeMarkdown(markdown: string): string {
  if (!markdown) return "";

  // Normalize Windows/Mac line endings to \n
  const normalized = markdown.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const lines = normalized.split("\n");
  const output: string[] = [];

  let inCodeBlock = false;
  let codeBlockLang = "";
  let codeBlockLines: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      if (inCodeBlock) {
        // Closing code block
        output.push(renderWidgetOrCodeBlock(codeBlockLang, codeBlockLines));
        inCodeBlock = false;
        codeBlockLang = "";
        codeBlockLines = [];
      } else {
        // Opening code block
        inCodeBlock = true;
        codeBlockLang = trimmed.slice(3).trim();
        codeBlockLines = [];
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockLines.push(line);
    } else {
      // Normal line outside code block
      const escaped = escapeHtml(line);
      output.push(parseInlineTokens(escaped));
    }
  }

  // Gracefully close streaming code block if still open
  if (inCodeBlock) {
    output.push(renderWidgetOrCodeBlock(codeBlockLang, codeBlockLines));
  }

  // Assemble HTML with newline handling:
  // Non-block lines are connected with <br/> (empty lines naturally create <br/><br/>)
  let html = "";
  for (let i = 0; i < output.length; i++) {
    const part = output[i];
    const isBlock =
      part.startsWith('<pre class="glint-code-block"') ||
      part.includes('class="glint-widget');
    const prevIsBlock =
      i > 0 &&
      (output[i - 1].startsWith('<pre class="glint-code-block"') ||
        output[i - 1].includes('class="glint-widget'));

    if (i === 0) {
      html += part;
    } else if (isBlock || prevIsBlock) {
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
