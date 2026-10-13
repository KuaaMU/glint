/**
 * Glint Generative UI Engine
 *
 * Renders structured interactive and visual micro-components directly in HUD:
 * - widget:compare (Architecture PK & trade-off comparison matrix)
 * - widget:flow (Step-by-step pipeline & state machine sequence)
 * - widget:metrics (Technical complexity & parameter badge grid)
 * - widget:callout (Important caveats & architectural tips)
 * - tool-call capsule (Live function calling status & expandable inspection)
 */

import { escapeHtml } from "./safe-markdown.ts";

export interface CompareWidgetData {
  title?: string;
  columns?: string[];
  rows?: string[][];
}

export interface FlowStep {
  title?: string;
  desc?: string;
  phase?: string;
}

export interface FlowWidgetData {
  title?: string;
  steps?: FlowStep[];
}

export interface MetricItem {
  label?: string;
  value?: string;
  tone?: "cyan" | "amber" | "rose" | "neutral" | string;
}

export interface MetricsWidgetData {
  title?: string;
  items?: MetricItem[];
}

export interface CalloutWidgetData {
  type?: "tip" | "warning" | "caution" | "note" | string;
  title?: string;
  content?: string;
}

/**
 * 尝试安全解析 JSON（容忍流式截断）
 */
function tryParseJson<T>(raw: string): T | null {
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

/**
 * 渲染对比矩阵 (widget:compare)
 */
export function renderCompareWidget(rawJson: string): string {
  const data = tryParseJson<CompareWidgetData>(rawJson);
  if (!data || !data.columns || !data.rows) {
    return `<div class="glint-widget-fallback"><code>${escapeHtml(rawJson)}</code></div>`;
  }

  const titleHtml = data.title
    ? `<div class="widget-header"><span class="widget-badge">对比矩阵</span><span class="widget-title">${escapeHtml(data.title)}</span></div>`
    : "";

  const ths = data.columns.map((c) => `<th>${escapeHtml(c)}</th>`).join("");
  const trs = data.rows
    .map(
      (r) =>
        `<tr>${r.map((cell, idx) => `<td class="${idx === 0 ? "col-lead" : ""}">${escapeHtml(cell)}</td>`).join("")}</tr>`
    )
    .join("");

  return `
    <div class="glint-widget glint-widget-compare">
      ${titleHtml}
      <div class="table-scroll-wrapper">
        <table class="compare-table">
          <thead><tr>${ths}</tr></thead>
          <tbody>${trs}</tbody>
        </table>
      </div>
    </div>
  `;
}

/**
 * 渲染时序流程图 (widget:flow)
 */
export function renderFlowWidget(rawJson: string): string {
  const data = tryParseJson<FlowWidgetData>(rawJson);
  if (!data || !data.steps) {
    return `<div class="glint-widget-fallback"><code>${escapeHtml(rawJson)}</code></div>`;
  }

  const titleHtml = data.title
    ? `<div class="widget-header"><span class="widget-badge">流程时序</span><span class="widget-title">${escapeHtml(data.title)}</span></div>`
    : "";

  const stepsHtml = data.steps
    .map((s, idx) => {
      const stepTitle = s.title || s.phase || `阶段 ${idx + 1}`;
      const stepDesc = s.desc || "";
      return `
        <div class="flow-step-item">
          <div class="step-indicator">
            <span class="step-num">${idx + 1}</span>
            ${idx < (data.steps?.length ?? 0) - 1 ? '<span class="step-line"></span>' : ""}
          </div>
          <div class="step-content">
            <div class="step-title">${escapeHtml(stepTitle)}</div>
            ${stepDesc ? `<div class="step-desc">${escapeHtml(stepDesc)}</div>` : ""}
          </div>
        </div>
      `;
    })
    .join("");

  return `
    <div class="glint-widget glint-widget-flow">
      ${titleHtml}
      <div class="flow-timeline">${stepsHtml}</div>
    </div>
  `;
}

/**
 * 渲染复杂度与特征徽章网格 (widget:metrics)
 */
export function renderMetricsWidget(rawJson: string): string {
  const data = tryParseJson<MetricsWidgetData>(rawJson);
  if (!data || !data.items) {
    return `<div class="glint-widget-fallback"><code>${escapeHtml(rawJson)}</code></div>`;
  }

  const titleHtml = data.title
    ? `<div class="widget-header"><span class="widget-badge">特性指标</span><span class="widget-title">${escapeHtml(data.title)}</span></div>`
    : "";

  const itemsHtml = data.items
    .map((item) => {
      const toneClass = `tone-${item.tone || "cyan"}`;
      return `
        <div class="metric-card ${toneClass}">
          <div class="metric-label">${escapeHtml(item.label || "")}</div>
          <div class="metric-val">${escapeHtml(item.value || "")}</div>
        </div>
      `;
    })
    .join("");

  return `
    <div class="glint-widget glint-widget-metrics">
      ${titleHtml}
      <div class="metrics-grid">${itemsHtml}</div>
    </div>
  `;
}

/**
 * 渲染警示与架构避坑卡片 (widget:callout)
 */
export function renderCalloutWidget(rawJson: string): string {
  const data = tryParseJson<CalloutWidgetData>(rawJson);
  if (!data || !data.content) {
    return `<div class="glint-widget-fallback"><code>${escapeHtml(rawJson)}</code></div>`;
  }

  const calloutType = data.type || "warning";
  const icon = calloutType === "tip" ? "💡" : calloutType === "caution" ? "🚨" : "⚠️";
  const title = data.title || (calloutType === "tip" ? "优化技巧" : "重要警示");

  return `
    <div class="glint-widget glint-widget-callout callout-${calloutType}">
      <div class="callout-icon">${icon}</div>
      <div class="callout-body">
        <div class="callout-title">${escapeHtml(title)}</div>
        <div class="callout-text">${escapeHtml(data.content)}</div>
      </div>
    </div>
  `;
}

/**
 * 工具调用 (Tool Call) 胶囊组件构建
 */
export function createToolCallElement(payload: {
  name: string;
  args: string;
  status: string;
  result?: string | null;
}): HTMLElement {
  const el = document.createElement("div");
  el.className = `glint-tool-capsule status-${payload.status}`;

  const header = document.createElement("div");
  header.className = "tool-capsule-header";

  const left = document.createElement("div");
  left.className = "tool-header-left";

  const icon = document.createElement("span");
  icon.className = "tool-pulse-icon";
  icon.textContent = payload.status === "calling" ? "⚡" : "✓";

  const label = document.createElement("span");
  label.className = "tool-label";
  label.textContent = `工具调用 · ${payload.name}`;

  left.append(icon, label);

  const right = document.createElement("div");
  right.className = "tool-header-right";

  const statusBadge = document.createElement("span");
  statusBadge.className = "tool-status-tag";
  statusBadge.textContent = payload.status === "calling" ? "调取中..." : "已完成";

  const toggleBtn = document.createElement("button");
  toggleBtn.className = "tool-expand-btn";
  toggleBtn.textContent = "展开详情";

  right.append(statusBadge, toggleBtn);
  header.append(left, right);

  const detailBox = document.createElement("div");
  detailBox.className = "tool-detail-box";
  detailBox.style.display = "none";

  const argsLine = document.createElement("div");
  argsLine.className = "tool-param-line";
  argsLine.textContent = `入参: ${payload.args}`;
  detailBox.appendChild(argsLine);

  if (payload.result) {
    const resultLine = document.createElement("div");
    resultLine.className = "tool-result-line";
    resultLine.textContent = `返回: ${payload.result}`;
    detailBox.appendChild(resultLine);
  }

  toggleBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    const isHidden = detailBox.style.display === "none";
    detailBox.style.display = isHidden ? "block" : "none";
    toggleBtn.textContent = isHidden ? "收起" : "展开详情";
  });

  el.append(header, detailBox);
  return el;
}
