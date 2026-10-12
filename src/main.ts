import { invoke, Channel, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface AppConfig {
  base_url?: string;
  api_key?: string;
  model?: string;
  always_on_top?: boolean;
}

interface GlintTriggerPayload {
  text?: string;
  window_title?: string | null;
}

interface StreamChunk {
  type: "Token" | "Done" | "Error";
  data?: string;
}

type ContentPart =
  | { type: "text"; text: string }
  | { type: "image_url"; image_url: { url: string } };

type MessageContent = string | ContentPart[];

interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: MessageContent;
}

interface QueryRequest {
  messages: ChatMessage[];
  api_key?: string | null;
  base_url?: string | null;
  model?: string | null;
  window_title?: string | null;
}

// DOM 元素引用
const explanationText = document.getElementById("explanation-text") as HTMLDivElement;
const statusIndicator = document.getElementById("status-indicator") as HTMLDivElement;
const followupInput = document.getElementById("followup-input") as HTMLInputElement;
const btnSendFollowup = document.getElementById("btn-send-followup") as HTMLButtonElement;
const btnCloseCard = document.getElementById("btn-close-card") as HTMLButtonElement;
const btnSettingsToggle = document.getElementById("btn-settings-toggle") as HTMLButtonElement;
const btnSnipToggle = document.getElementById("btn-snip-toggle") as HTMLButtonElement | null;
const hostBadge = document.getElementById("host-badge") as HTMLDivElement | null;
const hostTitle = document.getElementById("host-title") as HTMLSpanElement | null;
const cardHandle = document.querySelector(".card-anchor-handle") as HTMLElement;
const resizeGrip = document.getElementById("resize-grip") as HTMLElement;

// 会话状态管理
let conversationMessages: ChatMessage[] = [];
let activeSessionId = 0;
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
  let currentTurnText = "";

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
    explanationText.innerHTML = "";
  }

  onEvent.onmessage = (chunk: StreamChunk) => {
    // 丢弃已经过期会话的消息
    if (sessionId !== activeSessionId) return;

    if (chunk.type === "Token" && chunk.data) {
      currentTurnText += chunk.data;
      targetOutputEl.innerHTML = currentTurnText
        .replace(/\n\n/g, "<br/><br/>")
        .replace(/\n/g, "<br/>");
      explanationText.scrollTop = explanationText.scrollHeight;
    } else if (chunk.type === "Done") {
      statusIndicator.style.display = "none";
      conversationMessages.push({ role: "assistant", content: currentTurnText });
    } else if (chunk.type === "Error") {
      statusIndicator.style.display = "none";
      targetOutputEl.innerHTML += `<br/><span style="color:#EF4444;">${chunk.data || "请求出错"}</span>`;
    }
  };

  const req: QueryRequest = {
    messages,
    api_key: getStoredSetting("glint_api_key", ""),
    base_url: getStoredSetting("glint_base_url", "https://api.deepseek.com/v1"),
    model: getStoredSetting("glint_model", "deepseek-chat"),
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
  updateHostBadge(windowTitle);
  conversationMessages = [{ role: "user", content: text }];
  executeExplain(conversationMessages, activeSessionId);
}

// 开始多模态截图透视会话
function startNewMultimodalQuery(
  imageDataUrl: string,
  prompt?: string,
  windowTitle?: string | null
) {
  activeSessionId++;
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

  // 渲染截图缩略图横幅
  explanationText.innerHTML = `
    <div class="capture-preview-banner">
      <img src="${imageDataUrl}" alt="截屏预览" />
      <span class="preview-badge">✨ 视觉透视</span>
    </div>
    <div class="reply-turn"></div>
  `;
  const replyTurnEl = explanationText.querySelector(".reply-turn") as HTMLElement;

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

// 右下角微缩放把手交互
if (resizeGrip) {
  resizeGrip.addEventListener("mousedown", async (e) => {
    e.preventDefault();
    e.stopPropagation();

    if (!isTauri()) return;

    try {
      const currentWin = getCurrentWindow();
      const scaleFactor = await currentWin.scaleFactor();
      const startX = e.screenX;
      const startY = e.screenY;
      const physSize = await currentWin.innerSize();
      const startLogicalW = physSize.width / scaleFactor;
      const startLogicalH = physSize.height / scaleFactor;

      const onMouseMove = async (moveEvent: MouseEvent) => {
        const deltaX = moveEvent.screenX - startX;
        const deltaY = moveEvent.screenY - startY;
        const newWidth = Math.max(320, Math.min(800, startLogicalW + deltaX));
        const newHeight = Math.max(240, Math.min(900, startLogicalH + deltaY));
        await currentWin.setSize(new LogicalSize(Math.round(newWidth), Math.round(newHeight)));
      };

      const onMouseUp = async () => {
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
        try {
          const phys = await currentWin.innerSize();
          const logW = Math.round(phys.width / scaleFactor);
          const logH = Math.round(phys.height / scaleFactor);
          localStorage.setItem("glint_card_width", logW.toString());
          localStorage.setItem("glint_card_height", logH.toString());
        } catch (_) {}
      };

      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    } catch (err) {
      console.error("调整大小异常:", err);
    }
  });
}

// 键盘与输入监听
followupInput.addEventListener("keydown", (e) => {
  if (e.key === "Enter") sendFollowup();
});
btnSendFollowup.addEventListener("click", sendFollowup);

// 窗口控制与快捷键
btnCloseCard.addEventListener("click", async () => {
  if (isTauri()) {
    await invoke("hide_window");
  } else {
    explanationText.innerHTML = "已收起卡片。按 <kbd>Alt + Q</kbd> 重新唤出。";
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
  if (e.key === "Escape") {
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
      updateHostBadge(winTitle);
      activeSessionId++;
      conversationMessages = [];
      explanationText.innerHTML =
        "💡 <b>未检测到选中文本</b><br/><br/>请先在任意窗口（浏览器、PDF、编辑器等）中用鼠标选中文本，然后再按下 <kbd>Alt + Q</kbd> 即可一键透视。您也可以按 <kbd>Alt + W</kbd> 框选屏幕进行视觉透视。";
      statusIndicator.style.display = "none";
      followupInput.focus();
    }
  });
} else {
  explanationText.innerHTML =
    "请在任意软件中选中文本后按下 <kbd>Alt + Q</kbd>，或按下 <kbd>Alt + W</kbd> 进行截屏透视。";
}
