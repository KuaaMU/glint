/**
 * Glint Deep Module: StreamingRenderPipeline
 *
 * Encapsulates streaming token aggregation, safe segmented DOM rendering,
 * and in-place mounting of Tool Call capsules and Generative UI widgets.
 *
 * Invariant: Never destructively invalidates the entire container or mounted
 * tool capsules on incoming tokens.
 */

import { renderSafeMarkdown, emptyElement } from "./safe-markdown.ts";
import { createToolCallElement } from "./generative-ui.ts";
import type { ToolCallPayload } from "./history-store.ts";

export class StreamingRenderPipeline {
  private container: HTMLElement;
  private currentTurnOutputEl: HTMLElement | null = null;
  private currentTurnText = "";
  private toolElementsMap = new Map<string, HTMLElement>();
  /// `data-code` of the block whose "已复制" confirmation should survive a
  /// re-render; cleared once that block scrolls out of the DOM.
  private lastCopiedCode: string | null = null;

  public markCopied(encodedCode: string): void {
    this.lastCopiedCode = encodedCode;
  }

  constructor(container: HTMLElement) {
    this.container = container;
  }

  /**
   * 开启一个全新的对话回合或清空视窗
   */
  public beginTurn(options?: {
    isFollowup?: boolean;
    userQuerySummary?: string;
    targetOverride?: HTMLElement;
  }): HTMLElement {
    this.currentTurnText = "";
    this.toolElementsMap.clear();

    if (options?.targetOverride) {
      this.currentTurnOutputEl = options.targetOverride;
      return this.currentTurnOutputEl;
    }

    if (options?.isFollowup) {
      const qDiv = document.createElement("div");
      qDiv.style.cssText =
        "font-weight:600; color:var(--text-muted); margin:12px 0 6px 0; font-size:12px; border-top:1px dashed rgba(255,255,255,0.08); padding-top:8px;";
      qDiv.textContent = `追问: ${options.userQuerySummary || ""}`;
      this.container.appendChild(qDiv);

      const aDiv = document.createElement("div");
      aDiv.className = "reply-turn";
      this.container.appendChild(aDiv);
      this.currentTurnOutputEl = aDiv;
    } else {
      emptyElement(this.container);
      this.currentTurnOutputEl = this.container;
    }

    return this.currentTurnOutputEl;
  }

  /**
   * 追加流式文本 Token (增量安全渲染，不破坏 Tool 胶囊)
   */
  public appendToken(token: string): void {
    if (!this.currentTurnOutputEl) return;
    this.currentTurnText += token;

    // 安全渲染 Markdown 字符串
    this.currentTurnOutputEl.innerHTML = renderSafeMarkdown(this.currentTurnText);

    // 重新确保已挂载的 Tool 胶囊稳定插在当前回合顶部
    for (const toolEl of this.toolElementsMap.values()) {
      if (!this.currentTurnOutputEl.contains(toolEl)) {
        this.currentTurnOutputEl.insertBefore(toolEl, this.currentTurnOutputEl.firstChild);
      }
    }

    // Streaming replaces the markup on every token, which would leave an
    // already-shown "已复制" chip reset mid-read. Preserve the confirmation
    // state for the block whose text has not changed since it was copied.
    if (this.lastCopiedCode !== null) {
      const escaped = this.lastCopiedCode.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
      const selector = `.glint-copy-btn[data-code="${escaped}"]`;
      const restored = this.currentTurnOutputEl.querySelector<HTMLButtonElement>(selector);
      if (restored) {
        restored.textContent = "已复制";
        restored.classList.add("copied");
      } else {
        this.lastCopiedCode = null;
      }
    }

    this.container.scrollTop = this.container.scrollHeight;
  }

  /**
   * 挂载或更新工具调用胶囊 (Tool Call Capsule 原位稳定挂载)
   */
  public mountToolCall(payload: ToolCallPayload): void {
    if (!this.currentTurnOutputEl) return;

    const toolKey = `${payload.name}_${payload.args}`;
    const existingEl = this.toolElementsMap.get(toolKey);
    const newEl = createToolCallElement(payload);

    if (existingEl && this.currentTurnOutputEl.contains(existingEl)) {
      existingEl.replaceWith(newEl);
    } else {
      this.currentTurnOutputEl.insertBefore(newEl, this.currentTurnOutputEl.firstChild);
    }

    this.toolElementsMap.set(toolKey, newEl);
    this.container.scrollTop = this.container.scrollHeight;
  }

  /**
   * 呈现流式异常报错
   */
  public reportError(errMsg: string): void {
    if (!this.currentTurnOutputEl) return;
    const errNotice = document.createElement("div");
    errNotice.className = "stream-error-notice";
    errNotice.textContent = `⚠️ ${errMsg || "请求出错"}`;
    this.currentTurnOutputEl.appendChild(errNotice);
    this.container.scrollTop = this.container.scrollHeight;
  }

  /**
   * 获取当前回合生成的累积文本
   */
  public getCurrentText(): string {
    return this.currentTurnText;
  }
}
