import { invoke, Channel, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { renderSafeMarkdown, truncateSelection, emptyElement } from "./safe-markdown.ts";
import {
  HistoryStore,
  type GlintSessionRecord,
  type ChatMessage,
  type MessageContent,
  type ToolCallPayload,
} from "./history-store.ts";
import { StreamingRenderPipeline } from "./streaming-pipeline.ts";

export type GlintTheme = "dark" | "aurora" | "amethyst" | "epaper";

interface AppConfig {
  base_url?: string;
  api_key?: string;
  model?: string;
  theme?: GlintTheme | string;
  always_on_top?: boolean;
}

export function applyTheme(themeName?: string) {
  const chosen = (themeName || localStorage.getItem("glint_theme") || "dark") as GlintTheme;
  document.body.dataset.theme = chosen;
  document.documentElement.dataset.theme = chosen;
}
applyTheme();

interface GlintTriggerPayload {
  text?: string;
  window_title?: string | null;
}

interface StreamChunk {
  type: "Token" | "ToolCall" | "Done" | "Error";
  data?: string | ToolCallPayload;
}

interface QueryRequest {
  messages: ChatMessage[];
  api_key?: string | null;
  base_url?: string | null;
  model?: string | null;
  window_title?: string | null;
  enable_tools?: boolean;
}

// DOM 元素引用
const explanationText = document.getElementById("explanation-text") as HTMLDivElement;
const statusIndicator = document.getElementById("status-indicator") as HTMLDivElement;
const followupInput = document.getElementById("followup-input") as HTMLInputElement;
const btnSendFollowup = document.getElementById("btn-send-followup") as HTMLButtonElement;
const btnCloseCard = document.getElementById("btn-close-card") as HTMLButtonElement;
const btnSettingsToggle = document.getElementById("btn-settings-toggle") as HTMLButtonElement;
const btnSnipToggle = document.getElementById("btn-snip-toggle") as HTMLButtonElement | null;
const btnHistoryToggle = document.getElementById("btn-history-toggle") as HTMLButtonElement | null;
const historyDeck = document.getElementById("history-deck") as HTMLDivElement | null;
const btnCloseHistory = document.getElementById("btn-close-history") as HTMLButtonElement | null;
const historySearchInput = document.getElementById("history-search-input") as HTMLInputElement | null;
const historyList = document.getElementById("history-list") as HTMLDivElement | null;
const btnExportMarkdown = document.getElementById("btn-export-markdown") as HTMLButtonElement | null;
const btnClearHistory = document.getElementById("btn-clear-history") as HTMLButtonElement | null;

const hostBadge = document.getElementById("host-badge") as HTMLDivElement | null;
const hostTitle = document.getElementById("host-title") as HTMLSpanElement | null;
const cardHandle = document.querySelector(".card-anchor-handle") as HTMLElement;
const resizeGrip = document.getElementById("resize-grip") as HTMLElement;
const truncationCapsule = document.getElementById("truncation-capsule") as HTMLDivElement | null;
const truncationCapsuleText = document.getElementById("truncation-capsule-text") as HTMLSpanElement | null;

// 更新选中文本超限截断胶囊提示
function updateTruncationUI(truncated: boolean, originalLen: number) {
  if (!truncationCapsule) return;
  if (truncated) {
    if (truncationCapsuleText) {
      truncationCapsuleText.textContent = `已截取前 3,000 字符（原文共 ${originalLen.toLocaleString()} 字）· 保护上下文`;
    }
    truncationCapsule.style.display = "inline-flex";
  } else {
    truncationCapsule.style.display = "none";
  }
}

// 会话状态管理
let conversationMessages: ChatMessage[] = [];
let activeSessionId = 0;
let currentSessionId = "";
let currentSessionCreatedAt = 0;
let currentSessionIsMultimodal = false;
let currentSessionImageDataUrl = "";
let currentHostTitle: string | null = null;

// 配置状态获取
function getStoredSetting(key: string, fallback: string): string {
  return localStorage.getItem(key) || fallback;
}

// 更新顶部宿主应用感知胶囊 (避免将纯 UI 提示回写进 currentHostTitle)
function updateHostBadge(title: string | null | undefined, fallbackDisplay?: string) {
  currentHostTitle = title?.trim() || null;
  if (!hostBadge || !hostTitle) return;

  const raw = currentHostTitle || fallbackDisplay;
  if (raw && raw.trim()) {
    let display = raw.trim();
    if (display.includes("bilibili") || display.includes("哔哩哔哩")) {
      display = "哔哩哔哩 · bilibili";
    } else if (display.includes("Visual Studio Code")) {
      display = "VS Code";
    } else if (display.includes("GitHub")) {
      display = "GitHub";
    } else if (display.includes("Chrome") || display.includes("Edge")) {
      const parts = display.split(" - ");
      display = parts.length > 1 ? parts[0] : display;
    }
    if (display.length > 22) {
      display = display.slice(0, 20) + "...";
    }
    hostTitle.textContent = display;
    hostBadge.style.display = "inline-flex";
  } else {
    hostBadge.style.display = "none";
  }
}

// 提取用户消息纯文本概览用于 UI 显示
function getMessageDisplaySummary(content: MessageContent): string {
  if (typeof content === "string") return content;
  const textPart = content.find((p) => p.type === "text") as { type: "text"; text: string } | undefined;
  return textPart ? textPart.text : "【屏幕截图与标注】";
}

// 执行流式解释查询并绑定当前会话
async function executeExplain(
  messages: ChatMessage[],
  sessionId: number,
  targetElementOverride?: HTMLElement
) {
  if (sessionId !== activeSessionId) return;

  statusIndicator.style.display = "inline-flex";

  const onEvent = new Channel<StreamChunk>();

  // 决定当前轮次输出承载容器
  let targetOutputEl: HTMLElement = explanationText;
  if (targetElementOverride) {
    targetOutputEl = targetElementOverride;
  } else if (messages.length > 1) {
    const lastUserSummary = getMessageDisplaySummary(messages[messages.length - 1].content);
    const qDiv = document.createElement("div");
    qDiv.style.cssText =
      "font-weight:600; color:var(--text-muted); margin:12px 0 6px 0; font-size:12px; border-top:1px dashed rgba(255,255,255,0.08); padding-top:8px;";
    qDiv.textContent = `追问: ${lastUserSummary}`;
    explanationText.appendChild(qDiv);

    const aDiv = document.createElement("div");
    aDiv.className = "reply-turn";
    explanationText.appendChild(aDiv);
    targetOutputEl = aDiv;
  } else {
    emptyElement(explanationText);
  }

  // 创建并启动深模块流式渲染管线
  const pipeline = new StreamingRenderPipeline(targetOutputEl);
  activePipeline = pipeline;
  pipeline.beginTurn({ targetOverride: targetOutputEl });

  onEvent.onmessage = (chunk: StreamChunk) => {
    // 丢弃已经过期会话的消息
    if (sessionId !== activeSessionId) return;

    if (chunk.type === "Token" && typeof chunk.data === "string") {
      pipeline.appendToken(chunk.data);
    } else if (chunk.type === "ToolCall") {
      // Tool calls are implementation detail: keep the loading indicator and
      // never render a capsule. Users only need the answer text.
      statusIndicator.style.display = "inline-flex";
    } else if (chunk.type === "Done") {
      statusIndicator.style.display = "none";
      const fullTurnText = pipeline.getCurrentText();
      conversationMessages.push({ role: "assistant", content: fullTurnText });

      // 持久化保存至本地历史存储
      saveCurrentSessionToHistory(fullTurnText);
    } else if (chunk.type === "Error") {
      statusIndicator.style.display = "none";
      const errNotice = typeof chunk.data === "string" ? chunk.data : "请求出错";
      pipeline.reportError(errNotice);
    }
  };

  const req: QueryRequest = {
    messages,
    api_key: getStoredSetting("glint_api_key", ""),
    base_url: getStoredSetting("glint_base_url", "https://api.deepseek.com/v1"),
    model: getStoredSetting("glint_model", "deepseek-chat"),
    // First token must not wait for a tool round-trip.
    enable_tools: false,
    window_title: currentHostTitle,
  };

  if (isTauri()) {
    try {
      await invoke("explain", { req, onEvent });
    } catch (err) {
      if (sessionId === activeSessionId) {
        statusIndicator.style.display = "none";
        targetOutputEl.textContent = `调用错误: ${err}`;
      }
    }
  } else {
    // 浏览器独立预览模式模拟
    simulateMockStream(messages, onEvent, sessionId);
  }
}

// 自动持久化保存当前会话
function saveCurrentSessionToHistory(lastTurnOutput: string) {
  if (!currentSessionId) {
    currentSessionId = `glint_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
    currentSessionCreatedAt = Date.now();
  }

  const firstUserMsg = conversationMessages[0];
  const querySummary = firstUserMsg ? getMessageDisplaySummary(firstUserMsg.content) : "透视查询";
  const cleanSnippet = lastTurnOutput.replace(/```[\s\S]*?```/g, "").trim().slice(0, 100);

  HistoryStore.save({
    id: currentSessionId,
    createdAt: currentSessionCreatedAt || Date.now(),
    updatedAt: Date.now(),
    hostTitle: currentHostTitle,
    queryText: querySummary,
    isMultimodal: currentSessionIsMultimodal,
    imageDataUrl: currentSessionImageDataUrl || undefined,
    previewSnippet: cleanSnippet || lastTurnOutput.slice(0, 80),
    messages: [...conversationMessages],
  });
}

// 模拟流式输出 (用于纯前端浏览器环境)
async function simulateMockStream(
  messages: ChatMessage[],
  channel: Channel<StreamChunk>,
  sessionId: number
) {
  const lastUserSummary = getMessageDisplaySummary(messages[messages.length - 1].content);
  let reply = "";

  const hasImage = messages.some((m) => {
    if (Array.isArray(m.content)) {
      return m.content.some((p) => p.type === "image_url");
    }
    return false;
  });

  if (hasImage && messages.length === 1) {
    reply =
      "【视觉多模态透视】已定位屏幕焦点：\n1. 场景结构：所选区域包含当前窗口的核心交互意图与标注重点；\n2. 本质洞察：利用 Focus-First 渐进式透镜消除视觉干扰，直接聚合关键结论；\n3. 深入探讨：可在下方直接就该区域的某个细节继续追问。";
  } else if (lastUserSummary.includes("B-Tree") || lastUserSummary.includes("LSM") || lastUserSummary.includes("树")) {
    if (channel.onmessage) {
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "lookup_doc",
          args: JSON.stringify({ query: "B-Tree vs LSM-Tree", domain: "database" }),
          status: "calling",
        },
      });
      await new Promise((r) => setTimeout(r, 120));
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "lookup_doc",
          args: JSON.stringify({ query: "B-Tree vs LSM-Tree", domain: "database" }),
          status: "done",
          result: "B-Tree 原地更新点读极高，LSM-Tree 追加写吞吐极大。",
        },
      });
      await new Promise((r) => setTimeout(r, 60));
    }
    reply =
      "B-Tree 与 LSM-Tree 是现代存储引擎的两大基本派系，分别代表对点读与写吞吐的极致权衡：\n\n```widget:compare\n{\n  \"title\": \"存储引擎架构选型矩阵\",\n  \"columns\": [\"对比维度\", \"B-Tree (InnoDB / SQLite)\", \"LSM-Tree (RocksDB / ClickHouse)\"],\n  \"rows\": [\n    [\"写入模型\", \"原地更新 (In-place)\", \"追加写入 (Append-Only WAL)\"],\n    [\"读性能\", \"极快点读 O(log N)\", \"较慢点读 (需多层 SSTable 与布隆过滤器)\"],\n    [\"写放大与吞吐\", \"随机写易引起页分裂\", \"顺序高吞吐，后台Compaction整理\"]\n  ]\n}\n```\n\n- **B-Tree 核心优势**：适合读多写少、事务强一致性场景；\n- **LSM-Tree 核心优势**：适合海量写入、日志追加与分布式时序数据。";
  } else if (lastUserSummary.includes("0x") || lastUserSummary.includes(">>") || lastUserSummary.includes("<<")) {
    if (channel.onmessage) {
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "calc_sandbox",
          args: JSON.stringify({ expression: "0x1000 >> 4" }),
          status: "calling",
        },
      });
      await new Promise((r) => setTimeout(r, 100));
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "calc_sandbox",
          args: JSON.stringify({ expression: "0x1000 >> 4" }),
          status: "done",
          result: "4096 >> 4 = 256 (0x100)",
        },
      });
    }
    reply =
      "位运算沙箱已安全完成求值：\n\n```widget:metrics\n{\n  \"title\": \"数值转换与位拓扑\",\n  \"items\": [\n    {\"label\": \"十六进制 Hex\", \"value\": \"0x1000\", \"tone\": \"cyan\"},\n    {\"label\": \"十进制 Dec\", \"value\": \"4096\", \"tone\": \"cyan\"},\n    {\"label\": \"右移4位 >> 4\", \"value\": \"256 (0x100)\", \"tone\": \"amber\"},\n    {\"label\": \"二进制 Bin\", \"value\": \"0b100000000\", \"tone\": \"neutral\"}\n  ]\n}\n```";
  } else if (lastUserSummary.includes("零拷贝") || lastUserSummary.includes("Zero-Copy") || lastUserSummary.includes("mmap")) {
    if (channel.onmessage) {
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "lookup_doc",
          args: JSON.stringify({ query: "Zero-Copy sendfile", domain: "linux" }),
          status: "calling",
        },
      });
      await new Promise((r) => setTimeout(r, 100));
      channel.onmessage({
        type: "ToolCall",
        data: {
          name: "lookup_doc",
          args: JSON.stringify({ query: "Zero-Copy sendfile", domain: "linux" }),
          status: "done",
          result: "sendfile(2) / splice(2) 绕过内核态到用户态的重复拷贝。",
        },
      });
    }
    reply =
      "零拷贝的核心在于消除用户空间与内核空间的无意义搬运：\n\n```widget:flow\n{\n  \"title\": \"零拷贝 (Zero-Copy) 数据流转\",\n  \"steps\": [\n    {\"title\": \"1. 磁盘 DMA 读取\", \"desc\": \"数据由 DMA 控制器直接载入内核缓冲区\"},\n    {\"title\": \"2. 规避用户空间\", \"desc\": \"sendfile/splice 不再将数据复制到用户态内存\"},\n    {\"title\": \"3. 协议栈发送\", \"desc\": \"直接由内核传输至网卡缓冲区发送，切换次数从4次降为2次\"}\n  ]\n}\n```";
  } else if (messages.length > 1) {
    if (lastUserSummary.includes("GPU") || lastUserSummary.includes("显卡")) {
      reply =
        "在 GPU 架构中，这一矛盾更加尖锐：GPU 拥有成千上万个并发运算单元，如果全部向全局显存索取数据，总线会瞬间饱和。因此 GPU 引入了片上 Shared Memory（SRAM）。现代算子优化（如 FlashAttention）正是通过在局部 SRAM 内部完成计算，大幅减少往返显存的次数。";
    } else {
      reply = `针对追问“${lastUserSummary}”：在真实底层系统与当前应用场景中，核心权衡在于以局部缓存与空间换取更低的延迟与一致性损耗。`;
    }
  } else if (lastUserSummary.includes("下载客户端")) {
    reply =
      "在哔哩哔哩网页版当前场景中，这是引导安装桌面端独立客户端的入口。相比浏览器，客户端支持 4K 超清硬解加速、更低系统功耗、后台常驻并提供离线缓存下载。";
  } else {
    reply = `已直击核心词条“${lastUserSummary}”。结合当前应用情境，其关键逻辑在于消除抽象层假象，直面真实的底层机制。`;
  }

  for (const char of reply) {
    if (sessionId !== activeSessionId) return;
    if (channel.onmessage) {
      channel.onmessage({ type: "Token", data: char });
    }
    await new Promise((r) => setTimeout(r, 12));
  }
  if (sessionId === activeSessionId && channel.onmessage) {
    channel.onmessage({ type: "Done" });
  }
}

// 开始纯文本透视会话
function startNewQuery(text: string, windowTitle?: string | null) {
  activeSessionId++;
  currentSessionId = `glint_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
  currentSessionCreatedAt = Date.now();
  currentSessionIsMultimodal = false;
  currentSessionImageDataUrl = "";

  updateHostBadge(windowTitle);
  const truncatedResult = truncateSelection(text, 3000);
  updateTruncationUI(truncatedResult.truncated, truncatedResult.originalLen);
  conversationMessages = [{ role: "user", content: truncatedResult.text }];
  executeExplain(conversationMessages, activeSessionId);
}

// 开始多模态截图透视会话
function startNewMultimodalQuery(
  imageDataUrl: string,
  prompt?: string,
  windowTitle?: string | null
) {
  activeSessionId++;
  currentSessionId = `glint_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
  currentSessionCreatedAt = Date.now();
  currentSessionIsMultimodal = true;
  currentSessionImageDataUrl = imageDataUrl;

  updateTruncationUI(false, 0);
  updateHostBadge(windowTitle, "屏幕透视");
  const defaultPrompt = prompt || "请分析所选屏幕区域的内容，一针见血透视核心本质与关键信息。";
  conversationMessages = [
    {
      role: "user",
      content: [
        { type: "text", text: defaultPrompt },
        { type: "image_url", image_url: { url: imageDataUrl } },
      ],
    },
  ];

  // 渲染截图缩略图横幅 (安全 DOM 构建)
  emptyElement(explanationText);

  const banner = document.createElement("div");
  banner.className = "capture-preview-banner";

  const img = document.createElement("img");
  img.src = imageDataUrl;
  img.alt = "截屏预览";

  const badge = document.createElement("span");
  badge.className = "preview-badge";
  badge.textContent = "✨ 视觉透视";

  banner.appendChild(img);
  banner.appendChild(badge);

  const replyTurnEl = document.createElement("div");
  replyTurnEl.className = "reply-turn";

  explanationText.appendChild(banner);
  explanationText.appendChild(replyTurnEl);

  executeExplain(conversationMessages, activeSessionId, replyTurnEl);
  followupInput.focus();
}

// 发送深入追问
function sendFollowup() {
  const q = followupInput.value.trim();
  if (!q) return;
  followupInput.value = "";

  conversationMessages.push({ role: "user", content: q });
  executeExplain(conversationMessages, activeSessionId);
}

// 拖拽窗口
if (cardHandle) {
  cardHandle.addEventListener("mousedown", async (e) => {
    if (e.button === 0 && !(e.target as HTMLElement).closest(".glass-icon-btn")) {
      if (isTauri()) {
        try {
          await getCurrentWindow().startDragging();
        } catch (err) {
          console.error("拖拽异常:", err);
        }
      }
    }
  });
}

// 唤出截屏工具
if (btnSnipToggle) {
  btnSnipToggle.addEventListener("click", async () => {
    if (isTauri()) {
      try {
        await invoke("open_snipping_tool");
      } catch (err) {
        console.error("唤起截屏工具失败:", err);
      }
    } else {
      window.open("/snipping.html", "_blank");
    }
  });
}

// 右下角原生缩放把手交互（OS 操作系统原生拖拽托管，杜绝 IPC 逐帧阻塞）
if (resizeGrip) {
  resizeGrip.addEventListener("mousedown", async (e) => {
    e.preventDefault();
    e.stopPropagation();

    if (isTauri()) {
      try {
        await getCurrentWindow().startResizeDragging("SouthEast");
        return;
      } catch (err) {
        console.warn("原生窗口缩放托管失败，回退为 RAF 节流方案:", err);
      }
    }

    // 非 Tauri / 纯浏览器测试环境：requestAnimationFrame 严格节流降级
    let rafId: number | null = null;
    const startX = e.screenX;
    const startY = e.screenY;
    const startW = window.innerWidth;
    const startH = window.innerHeight;

    const onMouseMove = (moveEvent: MouseEvent) => {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        const deltaX = moveEvent.screenX - startX;
        const deltaY = moveEvent.screenY - startY;
        const newW = Math.max(320, Math.min(800, startW + deltaX));
        const newH = Math.max(240, Math.min(900, startH + deltaY));
        window.resizeTo?.(newW, newH);
      });
    };

    const onMouseUp = () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  });
}

// 监听视窗尺寸变化并防抖持久化（解耦鼠标移动与储存 IO）
let resizePersistTimer: number | null = null;
window.addEventListener("resize", () => {
  if (resizePersistTimer) clearTimeout(resizePersistTimer);
  resizePersistTimer = window.setTimeout(() => {
    try {
      const w = Math.round(window.innerWidth);
      const h = Math.round(window.innerHeight);
      if (w >= 320 && w <= 800 && h >= 240 && h <= 900) {
        localStorage.setItem("glint_card_width", w.toString());
        localStorage.setItem("glint_card_height", h.toString());
      }
    } catch (_) {}
  }, 250);
});

// 全局外部安全链接拦截器（调用 tauri-plugin-opener 在系统默认浏览器打开）
document.addEventListener("click", async (e) => {
  const target = (e.target as HTMLElement)?.closest?.("a.glint-link, a[target='_blank']");
  if (target && target instanceof HTMLAnchorElement && target.href) {
    e.preventDefault();
    if (isTauri()) {
      try {
        await openUrl(target.href);
      } catch (err) {
        console.error("打开外部链接失败:", err);
      }
    } else {
      window.open(target.href, "_blank");
    }
  }
});

// 键盘与输入监听
followupInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") sendFollowup();
});
btnSendFollowup.addEventListener("click", sendFollowup);

// 相对时间格式化
function formatRelativeTime(timestamp: number): string {
  const diffSec = Math.floor((Date.now() - timestamp) / 1000);
  if (diffSec < 60) return "刚刚";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin}分钟前`;
  const diffHour = Math.floor(diffMin / 60);
  if (diffHour < 24) return `${diffHour}小时前`;
  const diffDay = Math.floor(diffHour / 24);
  if (diffDay < 7) return `${diffDay}天前`;
  return new Date(timestamp).toLocaleDateString();
}

// 渲染历史记录列表
function renderHistoryList(filterKeyword?: string) {
  if (!historyList) return;
  emptyElement(historyList);

  const sessions = filterKeyword ? HistoryStore.search(filterKeyword) : HistoryStore.getAll();
  if (sessions.length === 0) {
    const tip = document.createElement("div");
    tip.className = "history-empty-tip";
    tip.textContent = filterKeyword ? "未搜索到匹配的历史记录" : "暂无历史记录 · 划词透视后将自动归档";
    historyList.appendChild(tip);
    return;
  }

  for (const sess of sessions) {
    const item = document.createElement("div");
    item.className = "history-item";

    const top = document.createElement("div");
    top.className = "history-item-top";

    const timeSpan = document.createElement("span");
    timeSpan.className = "history-time";
    timeSpan.textContent = formatRelativeTime(sess.createdAt);
    top.appendChild(timeSpan);

    if (sess.hostTitle) {
      const hostSpan = document.createElement("span");
      hostSpan.className = "history-host-tag";
      hostSpan.textContent = sess.hostTitle;
      top.appendChild(hostSpan);
    }

    const queryDiv = document.createElement("div");
    queryDiv.className = "history-query";
    queryDiv.textContent = sess.queryText;

    const snippetDiv = document.createElement("div");
    snippetDiv.className = "history-snippet";
    snippetDiv.textContent = sess.previewSnippet || "暂无摘要预览";

    const delBtn = document.createElement("button");
    delBtn.className = "history-item-del-btn";
    delBtn.title = "删除此条会话";
    delBtn.textContent = "✕";
    delBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      HistoryStore.delete(sess.id);
      renderHistoryList(historySearchInput?.value);
    });

    item.append(top, queryDiv, snippetDiv, delBtn);

    item.addEventListener("click", () => {
      restoreSession(sess);
    });

    historyList.appendChild(item);
  }
}

// 恢复历史会话到主视窗
function restoreSession(session: GlintSessionRecord) {
  activeSessionId++;
  currentSessionId = session.id;
  currentSessionCreatedAt = session.createdAt;
  currentSessionIsMultimodal = !!session.isMultimodal;
  currentSessionImageDataUrl = session.imageDataUrl || "";

  updateHostBadge(session.hostTitle);
  conversationMessages = [...session.messages];
  updateTruncationUI(false, 0);

  emptyElement(explanationText);

  // 恢复截图预览横幅
  if (session.isMultimodal && session.imageDataUrl) {
    const banner = document.createElement("div");
    banner.className = "capture-preview-banner";
    const img = document.createElement("img");
    img.src = session.imageDataUrl;
    img.alt = "截屏预览";
    const badge = document.createElement("span");
    badge.className = "preview-badge";
    badge.textContent = "✨ 视觉透视";
    banner.append(img, badge);
    explanationText.appendChild(banner);
  }

  // 重新渲染全部问答回合
  for (let i = 0; i < conversationMessages.length; i++) {
    const msg = conversationMessages[i];
    if (msg.role === "user") {
      if (i > 0) {
        const qDiv = document.createElement("div");
        qDiv.style.cssText =
          "font-weight:600; color:var(--text-muted); margin:12px 0 6px 0; font-size:12px; border-top:1px dashed rgba(255,255,255,0.08); padding-top:8px;";
        qDiv.textContent = `追问: ${getMessageDisplaySummary(msg.content)}`;
        explanationText.appendChild(qDiv);
      }
    } else if (msg.role === "assistant") {
      const aDiv = document.createElement("div");
      if (i > 1) aDiv.className = "reply-turn";
      aDiv.innerHTML = renderSafeMarkdown(typeof msg.content === "string" ? msg.content : "");
      explanationText.appendChild(aDiv);
    }
  }

  toggleHistoryDeck(false);
  followupInput.focus();
}

// 打开或关闭历史漫游透镜板
function toggleHistoryDeck(show?: boolean) {
  if (!historyDeck) return;
  const isCurrentlyOpen = historyDeck.style.display !== "none";
  const shouldOpen = show !== undefined ? show : !isCurrentlyOpen;

  if (shouldOpen) {
    if (historySearchInput) historySearchInput.value = "";
    renderHistoryList();
    historyDeck.style.display = "flex";
    historySearchInput?.focus();
  } else {
    historyDeck.style.display = "none";
  }
}

// 绑定历史透镜板交互
if (btnHistoryToggle) {
  btnHistoryToggle.addEventListener("click", () => toggleHistoryDeck());
}
if (btnCloseHistory) {
  btnCloseHistory.addEventListener("click", () => toggleHistoryDeck(false));
}
if (historySearchInput) {
  historySearchInput.addEventListener("input", (e) => {
    const val = (e.target as HTMLInputElement).value;
    renderHistoryList(val);
  });
}
if (btnExportMarkdown) {
  btnExportMarkdown.addEventListener("click", async () => {
    const md = HistoryStore.exportToMarkdown();
    try {
      await navigator.clipboard.writeText(md);
      const originText = btnExportMarkdown.textContent || "导出 Markdown 笔记";
      btnExportMarkdown.textContent = "✓ 已复制笔记";
      setTimeout(() => {
        btnExportMarkdown.textContent = originText;
      }, 2000);
    } catch {
      alert("复制到剪贴板失败，请检查应用权限。");
    }
  });
}
if (btnClearHistory) {
  btnClearHistory.addEventListener("click", () => {
    if (confirm("确定清空全部历史会话记录吗？")) {
      HistoryStore.clear();
      renderHistoryList();
    }
  });
}

// --- Code block copy (event delegation) -----------------------------------
let activePipeline: StreamingRenderPipeline | null = null;

// The streaming pipeline replaces `innerHTML` on every token, so per-block
// listeners would be destroyed mid-stream. A single delegated handler on the
// answer container survives re-renders.
explanationText.addEventListener("click", async (event) => {
  const target = event.target as HTMLElement | null;
  const button = target?.closest<HTMLButtonElement>(".glint-copy-btn");
  if (!button) return;

  const encoded = button.dataset.code;
  if (!encoded) return;

  let code = "";
  try {
    code = decodeURIComponent(encoded);
  } catch {
    return;
  }

  try {
    await navigator.clipboard.writeText(code);
  } catch {
    // Clipboard API can be unavailable in an unfocused webview; fall back to
    // a temporary textarea so copy still works.
    const area = document.createElement("textarea");
    area.value = code;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }

  activePipeline?.markCopied(encoded);
  const previous = button.textContent;
  button.textContent = "已复制";
  button.classList.add("copied");
  window.setTimeout(() => {
    button.textContent = previous || "复制";
    button.classList.remove("copied");
  }, 1400);
});

// --- Pin (固定) -----------------------------------------------------------
// A pinned card stays on screen and ignores the automatic hide that follows a
// trigger elsewhere. State is mirrored to the shell so `toggle_hud_window` can
// respect it, and persisted per user.
const PIN_STORAGE_KEY = "glint_pinned";
let isPinned = localStorage.getItem(PIN_STORAGE_KEY) === "true";
const btnPinCard = document.getElementById("btn-pin-card") as HTMLButtonElement | null;

function applyPinUI() {
  if (!btnPinCard) return;
  btnPinCard.setAttribute("aria-pressed", isPinned ? "true" : "false");
  btnPinCard.title = isPinned ? "已固定（点击取消）" : "固定卡片（不自动收起）";
  document.body.dataset.pinned = isPinned ? "true" : "false";
}

async function setPinned(next: boolean) {
  isPinned = next;
  localStorage.setItem(PIN_STORAGE_KEY, String(next));
  applyPinUI();
  if (isTauri()) {
    try {
      await invoke("set_hud_pinned", { pinned: next });
    } catch (err) {
      console.error("设置固定状态失败:", err);
    }
  }
}

applyPinUI();
btnPinCard?.addEventListener("click", () => {
  void setPinned(!isPinned);
});

// 窗口控制与快捷键
btnCloseCard.addEventListener("click", async () => {
  if (isTauri()) {
    await invoke("hide_window");
  } else {
    updateTruncationUI(false, 0);
    emptyElement(explanationText);
    const kbd = document.createElement("kbd");
    kbd.textContent = "Alt + Q";
    explanationText.append(
      document.createTextNode("已收起卡片。按 "),
      kbd,
      document.createTextNode(" 重新唤出。")
    );
  }
});

// 打开独立后台设置视窗
btnSettingsToggle.addEventListener("click", async () => {
  if (isTauri()) {
    try {
      await invoke("open_preferences_window");
    } catch (err) {
      console.error("打开偏好设置失败:", err);
    }
  } else {
    window.open("/preferences.html", "_blank");
  }
});

window.addEventListener("keydown", async (e) => {
  if ((e.ctrlKey || e.metaKey) && (e.key === "h" || e.key === "H")) {
    e.preventDefault();
    toggleHistoryDeck();
    return;
  }

  if (e.key === "Escape") {
    if (historyDeck && historyDeck.style.display !== "none") {
      e.preventDefault();
      toggleHistoryDeck(false);
      return;
    }
    // Escape always dismisses, even when pinned — pin only suppresses
    // automatic hiding, never an explicit user action.
    if (isTauri()) {
      await invoke("hide_window");
    }
  }
});

// 恢复持久化记忆的卡片视窗尺寸
async function restoreWindowSize() {
  if (!isTauri()) return;
  const savedW = localStorage.getItem("glint_card_width");
  const savedH = localStorage.getItem("glint_card_height");
  if (savedW && savedH) {
    try {
      const w = parseInt(savedW);
      const h = parseInt(savedH);
      if (w >= 320 && w <= 800 && h >= 240 && h <= 900) {
        await getCurrentWindow().setSize(new LogicalSize(w, h));
      }
    } catch (e) {
      console.warn("恢复卡片尺寸异常:", e);
    }
  }
}
restoreWindowSize();

// 启动时主动从后端配置同步
async function syncConfigFromBackend() {
  if (!isTauri()) return;
  try {
    const cfg = await invoke<AppConfig>("get_app_config");
    if (cfg) {
      if (cfg.base_url) localStorage.setItem("glint_base_url", cfg.base_url);
      if (cfg.api_key) localStorage.setItem("glint_api_key", cfg.api_key);
      if (cfg.model) localStorage.setItem("glint_model", cfg.model);
      if (cfg.theme) {
        localStorage.setItem("glint_theme", cfg.theme);
        applyTheme(cfg.theme);
      }
      if (cfg.always_on_top !== undefined) {
        localStorage.setItem("glint_always_on_top", cfg.always_on_top.toString());
      }
    }
  } catch (err) {
    console.warn("同步后端配置失败:", err);
  }
}
syncConfigFromBackend();

// 监听跨视窗配置实时同步事件
if (isTauri()) {
  listen<AppConfig>("glint-settings-changed", (event) => {
    const cfg = event.payload;
    if (cfg) {
      if (cfg.base_url) localStorage.setItem("glint_base_url", cfg.base_url);
      if (cfg.api_key !== undefined) localStorage.setItem("glint_api_key", cfg.api_key);
      if (cfg.model) localStorage.setItem("glint_model", cfg.model);
      if (cfg.theme) {
        localStorage.setItem("glint_theme", cfg.theme);
        applyTheme(cfg.theme);
      }
      if (cfg.always_on_top !== undefined) {
        localStorage.setItem("glint_always_on_top", cfg.always_on_top.toString());
      }
    }
  });

  // 监听卡片尺寸重置通知
  listen("glint-reset-window-size", async () => {
    try {
      await getCurrentWindow().setSize(new LogicalSize(440, 380));
      localStorage.setItem("glint_card_width", "440");
      localStorage.setItem("glint_card_height", "380");
    } catch (_) {}
  });

  // 监听截屏多模态透视触发事件
  listen<{ image_data_url: string; prompt?: string; window_title?: string | null }>(
    "glint-multimodal-trigger",
    (event) => {
      const payload = event.payload;
      if (payload && payload.image_data_url) {
        startNewMultimodalQuery(payload.image_data_url, payload.prompt, payload.window_title);
      }
    }
  );

  // 注册 Tauri 原生划词触发事件
  listen<GlintTriggerPayload | string>("glint-trigger", (event) => {
    let text = "";
    let winTitle: string | null = null;
    if (typeof event.payload === "string") {
      text = event.payload.trim();
    } else if (event.payload && typeof event.payload === "object") {
      text = (event.payload.text || "").trim();
      winTitle = event.payload.window_title || null;
    }

    if (text) {
      startNewQuery(text, winTitle);
    } else {
      updateTruncationUI(false, 0);
      updateHostBadge(winTitle);
      activeSessionId++;
      conversationMessages = [];
      emptyElement(explanationText);

      const titleDiv = document.createElement("div");
      titleDiv.style.marginBottom = "8px";
      const bold = document.createElement("b");
      bold.textContent = "未检测到选中文本";
      titleDiv.append(document.createTextNode("💡 "), bold);

      const descDiv = document.createElement("div");
      const kbdQ = document.createElement("kbd");
      kbdQ.textContent = "Alt + Q";
      const kbdW = document.createElement("kbd");
      kbdW.textContent = "Alt + W";
      descDiv.append(
        document.createTextNode("请先在任意窗口（浏览器、PDF、编辑器等）中用鼠标选中文本，然后再按下 "),
        kbdQ,
        document.createTextNode(" 即可一键透视。您也可以按 "),
        kbdW,
        document.createTextNode(" 框选屏幕进行视觉透视。")
      );

      explanationText.append(titleDiv, descDiv);
      statusIndicator.style.display = "none";
      followupInput.focus();
    }
  });
} else {
  updateTruncationUI(false, 0);
  emptyElement(explanationText);
  const kbdQ = document.createElement("kbd");
  kbdQ.textContent = "Alt + Q";
  const kbdW = document.createElement("kbd");
  kbdW.textContent = "Alt + W";
  explanationText.append(
    document.createTextNode("请在任意软件中选中文本后按下 "),
    kbdQ,
    document.createTextNode("，或按下 "),
    kbdW,
    document.createTextNode(" 进行截屏透视。")
  );
}
