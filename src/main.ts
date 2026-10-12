import { invoke, Channel, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

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
}

// DOM 元素引用
const explanationText = document.getElementById("explanation-text") as HTMLDivElement;
const statusIndicator = document.getElementById("status-indicator") as HTMLDivElement;
const followupInput = document.getElementById("followup-input") as HTMLInputElement;
const btnSendFollowup = document.getElementById("btn-send-followup") as HTMLButtonElement;
const btnCloseCard = document.getElementById("btn-close-card") as HTMLButtonElement;
const btnSettingsToggle = document.getElementById("btn-settings-toggle") as HTMLButtonElement;
const btnSnipToggle = document.getElementById("btn-snip-toggle") as HTMLButtonElement | null;
const cardHandle = document.querySelector(".card-anchor-handle") as HTMLElement;
const resizeGrip = document.getElementById("resize-grip") as HTMLElement;

// 多轮对话上下文记忆与独立会话隔离
let conversationMessages: ChatMessage[] = [];
let activeSessionId = 0;

// 配置状态获取
function getStoredSetting(key: string, fallback: string): string {
  return localStorage.getItem(key) || fallback;
}

// 提取用户消息纯文本概览用于 UI 显示
function getMessageDisplaySummary(content: MessageContent): string {
  if (typeof content === "string") return content;
  const textPart = content.find((p) => p.type === "text") as { type: "text"; text: string } | undefined;
  return textPart ? textPart.text : "【屏幕截图与标注】";
}

// 核心流式查询调用 (严格绑定 sessionId，防止幽灵脏数据串台)
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
      "font-weight:600; color:var(--text-muted); margin:12px 0 6px 0; font-size:12.5px; border-top:1px dashed var(--card-border); padding-top:8px;";
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
      "【视觉多模态透视】已解析所选屏幕区域：\n1. 核心结构：画面包含关键界面与标注指引，直观展示了系统交互与状态拓扑；\n2. 本质洞察：该设计采用最小心智负担的 Focus-First 渐进显露模式，避免传统工具栏信息过载；\n3. 建议与下步：您可在下方输入框针对画面中具体细节继续深入追问。";
  } else if (messages.length > 1) {
    if (lastUserSummary.includes("GPU") || lastUserSummary.includes("显卡")) {
      reply =
        "在 GPU 架构中，这一矛盾更加尖锐：GPU 拥有成千上万个并发运算单元，如果全部向全局显存索取数据，总线会瞬间饱和。因此 GPU 引入了片上 Shared Memory（SRAM）。现代算子优化（如 FlashAttention）正是通过在局部 SRAM 内部完成计算，大幅减少往返显存的次数。";
    } else if (lastUserSummary.includes("优化") || lastUserSummary.includes("怎么解决")) {
      reply =
        "在工程层面的核心思路包括：1. 结构体对齐（Padding/Align）：按 64 字节 Cache Line 对齐避免伪共享；2. 算子融合（Operator Fusion）：消除跨核反复搬移；3. 内存预取（Prefetching）：提前将下一循环的数据滑入缓存。";
    } else {
      reply = `针对你的追问“${lastUserSummary}”：在真实底层系统中，解决这一问题必须打破高级语言‘内存是平坦大数组’的假设，主动顺应硬件层级拓扑来组织数据。`;
    }
  } else {
    reply = lastUserSummary.includes("面对的并不是")
      ? "这里的关键在于「并不是」引出的转折：CPU 和 NPU/GPU 等运算核心面对的物理内存并非理想化的均匀整体。前文提到的虚拟地址空间是软件层面的抽象，给程序一种拥有连续、独占内存的假象；而一旦落到真实硬件，内存实际上是分层且不同质的——从极快但极小的寄存器、各级 Cache，到较慢的主存（DRAM），甚至跨芯片访问时还有更慢的一致性总线。因此，「一块均质平坦、可以瞬时读写的物理内存」是一种需要被打破的误解，这正是理解内存墙和 Cache 一致性的出发点。"
      : `已锁定核心词条“${lastUserSummary}”。其本质在于通过底层微架构级流水线优化，消解多线程资源竞争与内存延迟停顿。`;
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
function startNewQuery(text: string) {
  activeSessionId++;
  conversationMessages = [{ role: "user", content: text }];
  executeExplain(conversationMessages, activeSessionId);
}

// 开始多模态截图透视会话
function startNewMultimodalQuery(imageDataUrl: string, prompt?: string) {
  activeSessionId++;
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
    if (e.button === 0 && !(e.target as HTMLElement).closest(".icon-btn")) {
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

// 右下角微缩放把手交互 (贴合窗口边沿，高DPI自适应，无死区)
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
    const cfg: any = await invoke("get_app_config");
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
  listen<any>("glint-settings-changed", (event) => {
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
  listen<{ image_data_url: string; prompt?: string }>("glint-multimodal-trigger", (event) => {
    const payload = event.payload;
    if (payload && payload.image_data_url) {
      startNewMultimodalQuery(payload.image_data_url, payload.prompt);
    }
  });

  // 注册 Tauri 原生划词触发事件
  listen<string>("glint-trigger", (event) => {
    const text = event.payload ? event.payload.trim() : "";
    if (text) {
      startNewQuery(text);
    } else {
      // 哨兵探测判定无新选中文本：绝不显示旧内容，彻底重置会话
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
