import fs from "node:fs";
import path from "node:path";
import { PROJECT_ROOT, SRC_DIR, README_PATH, GLINT_HUD_DIR, SRC_TAURI_DIR } from "../test_config.mjs";

/**
 * Authoritative specification oracle for safe HTML escaping.
 * Standard: PROJECT.md § Interface Contracts (1. Frontend Markdown & Sanitization Contract)
 */
export function escapeHtmlOracle(raw) {
  if (!raw) return "";
  return String(raw)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/**
 * Authoritative specification oracle for inline token parsing (inline code `...` and bold **...**).
 */
export function parseInlineTokensOracle(escapedText) {
  if (!escapedText) return "";
  let result = "";
  let i = 0;
  const len = escapedText.length;

  while (i < len) {
    if (escapedText[i] === "`") {
      const closeIdx = escapedText.indexOf("`", i + 1);
      if (closeIdx !== -1) {
        const codeContent = escapedText.slice(i + 1, closeIdx);
        result += `<code class="glint-inline-code">${codeContent}</code>`;
        i = closeIdx + 1;
        continue;
      }
    }

    if (escapedText.startsWith("**", i)) {
      const closeIdx = escapedText.indexOf("**", i + 2);
      if (closeIdx !== -1 && closeIdx > i + 2) {
        const boldContent = escapedText.slice(i + 2, closeIdx);
        result += `<strong>${parseInlineTokensOracle(boldContent)}</strong>`;
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
 * Authoritative specification oracle for safe Markdown rendering.
 */
export function renderSafeMarkdownOracle(markdown) {
  if (!markdown) return "";

  const normalized = markdown.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const lines = normalized.split("\n");
  const output = [];

  let inCodeBlock = false;
  let codeBlockLang = "";
  let codeBlockLines = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed.startsWith("```")) {
      if (inCodeBlock) {
        const escapedCode = codeBlockLines.map((l) => escapeHtmlOracle(l)).join("\n");
        const langAttr = codeBlockLang ? ` data-lang="${escapeHtmlOracle(codeBlockLang)}"` : ` data-lang=""`;
        output.push(`<pre class="glint-code-block"><code${langAttr}>${escapedCode}</code></pre>`);
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
    } else {
      const escaped = escapeHtmlOracle(line);
      output.push(parseInlineTokensOracle(escaped));
    }
  }

  if (inCodeBlock) {
    const escapedCode = codeBlockLines.map((l) => escapeHtmlOracle(l)).join("\n");
    const langAttr = codeBlockLang ? ` data-lang="${escapeHtmlOracle(codeBlockLang)}"` : ` data-lang=""`;
    output.push(`<pre class="glint-code-block"><code${langAttr}>${escapedCode}</code></pre>`);
  }

  let html = "";
  for (let i = 0; i < output.length; i++) {
    const part = output[i];
    const isPre = part.startsWith('<pre class="glint-code-block"');
    const prevIsPre = i > 0 && output[i - 1].startsWith('<pre class="glint-code-block"');

    if (i === 0) {
      html += part;
    } else if (isPre || prevIsPre) {
      html += part;
    } else {
      html += "<br/>" + part;
    }
  }

  return html;
}

/**
 * Authoritative specification oracle for Unicode-aware truncation.
 */
export function truncateSelectionOracle(text, maxLen = 3000) {
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
 * Dynamically loads live safe-markdown module if available in src/.
 */
export async function loadLiveSafeMarkdownModule() {
  const tsPath = path.join(SRC_DIR, "safe-markdown.ts");
  if (!fs.existsSync(tsPath)) {
    return null;
  }
  try {
    // In Node 22 with --experimental-strip-types, direct import of .ts works
    const fileUrl = new URL(`file://${tsPath.replace(/\\/g, "/")}`);
    const mod = await import(fileUrl.href);
    return mod;
  } catch {
    return null;
  }
}

/**
 * Inspects src/main.ts for any naked innerHTML occurrences.
 */
export function scanMainTsForInnerHTML() {
  const mainTsPath = path.join(SRC_DIR, "main.ts");
  if (!fs.existsSync(mainTsPath)) {
    return { exists: false, violations: [] };
  }
  const content = fs.readFileSync(mainTsPath, "utf-8");
  const lines = content.split("\n");
  const violations = [];

  for (let idx = 0; idx < lines.length; idx++) {
    const line = lines[idx];
    // Check for property assignment: .innerHTML = or .innerHTML +=
    if (/\.innerHTML\s*\+?=/i.test(line)) {
      // Naked innerHTML write is one that does NOT pass through renderSafeMarkdown
      const isSanitized = /renderSafeMarkdown|escapeHtml/i.test(line);
      if (!isSanitized) {
        violations.push({
          line: idx + 1,
          content: line.trim(),
        });
      }
    }
  }

  return { exists: true, violations };
}

/**
 * Inspects README.md for platform matrix and shortcut declarations.
 */
export function inspectReadmeContent() {
  if (!fs.existsSync(README_PATH)) {
    return { exists: false };
  }
  const content = fs.readFileSync(README_PATH, "utf-8");
  return {
    exists: true,
    hasWindowsTier1: /Windows.*(?:Tier 1|主力生产)/i.test(content),
    hasMacOsExperimental: /macOS.*(?:Experimental|实验性)/i.test(content),
    hasLinuxExperimental: /Linux.*(?:Experimental|实验性)/i.test(content),
    hasAltWShortcut: /Alt\s*\+\s*W/i.test(content),
    hasAltQShortcut: /Alt\s*\+\s*Q/i.test(content),
    hasAltDShortcut: /Alt\s*\+\s*D/i.test(content),
  };
}

/**
 * Checks Slint HUD contract properties in crates/glint-hud/ui/hud.slint.
 */
export function inspectSlintHudContract() {
  const hudSlintPath = path.join(GLINT_HUD_DIR, "ui", "hud.slint");
  if (!fs.existsSync(hudSlintPath)) {
    return { exists: false };
  }
  const content = fs.readFileSync(hudSlintPath, "utf-8");
  return {
    exists: true,
    hasNoFrame: /no-frame:\s*true/i.test(content),
    hasTransparentBackground: /background:\s*transparent/i.test(content),
    hasObsidianGlassBg: /#0d1117/i.test(content),
    hasLightBlade: /linear-gradient\(90deg/i.test(content) || /height:\s*1px/i.test(content),
    hasBreathingGlow: /glow_pulse/i.test(content) || /opacity:/i.test(content),
    hasWindowTitleProperty: /active_window_title|host-window-title/i.test(content),
    hasStreamingProperty: /is_streaming|is-streaming/i.test(content),
    hasQueryInput: /query_input|submit-followup|submit_query/i.test(content),
  };
}
