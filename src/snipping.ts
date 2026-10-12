import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface ScreenGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ScreenCaptureResult {
  image_data_url: string;
  geometry: ScreenGeometry;
}

interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

type DrawTool = "pen" | "rect" | "arrow";

interface Stroke {
  tool: DrawTool;
  color: string;
  points?: Array<{ x: number; y: number }>;
  start?: { x: number; y: number };
  end?: { x: number; y: number };
}

// DOM 元素引用
const container = document.getElementById("snipping-container") as HTMLDivElement;
const bgCanvas = document.getElementById("bg-canvas") as HTMLCanvasElement;
const maskCanvas = document.getElementById("mask-canvas") as HTMLCanvasElement;
const drawCanvas = document.getElementById("draw-canvas") as HTMLCanvasElement;
const selectionBox = document.getElementById("selection-box") as HTMLDivElement;
const dimBadge = document.getElementById("dim-badge") as HTMLDivElement;
const focusPill = document.getElementById("focus-pill") as HTMLDivElement;
const btnSubmitExplain = document.getElementById("btn-submit-explain") as HTMLButtonElement;
const btnToggleDraw = document.getElementById("btn-toggle-draw") as HTMLButtonElement;
const btnCancel = document.getElementById("btn-cancel") as HTMLButtonElement;
const drawDrawer = document.getElementById("draw-drawer") as HTMLDivElement;
const btnUndoDraw = document.getElementById("btn-undo-draw") as HTMLButtonElement;
const snipHint = document.getElementById("snip-hint") as HTMLDivElement;

const bgCtx = bgCanvas.getContext("2d")!;
const maskCtx = maskCanvas.getContext("2d")!;
const drawCtx = drawCanvas.getContext("2d")!;

// 状态管理
let bgImage: HTMLImageElement | null = null;
let currentSelection: Rect | null = null;
let isSelecting = false;
let isMoving = false;
let isResizing = false;
let activeResizeHandle = "";
let startPoint = { x: 0, y: 0 };
let moveOffset = { x: 0, y: 0 };
let originalSelection: Rect = { x: 0, y: 0, width: 0, height: 0 };

// 标注状态
let isDrawMode = false;
let currentTool: DrawTool = "pen";
let currentColor = "#38bdf8";
let strokes: Stroke[] = [];
let currentStroke: Stroke | null = null;
let isDrawing = false;

// 设备像素比与视窗尺寸
let dpr = window.devicePixelRatio || 1;
let viewW = window.innerWidth;
let viewH = window.innerHeight;

function resizeCanvases() {
  dpr = window.devicePixelRatio || 1;
  viewW = window.innerWidth;
  viewH = window.innerHeight;

  for (const c of [bgCanvas, maskCanvas, drawCanvas]) {
    c.width = viewW * dpr;
    c.height = viewH * dpr;
    c.style.width = `${viewW}px`;
    c.style.height = `${viewH}px`;
  }

  bgCtx.scale(dpr, dpr);
  maskCtx.scale(dpr, dpr);
  drawCtx.scale(dpr, dpr);

  redrawAll();
}

// 绘制底图
function drawBackgroundImage() {
  bgCtx.clearRect(0, 0, viewW, viewH);
  if (bgImage && bgImage.complete && bgImage.naturalWidth > 0) {
    bgCtx.drawImage(bgImage, 0, 0, viewW, viewH);
  } else {
    // 浏览器独立预览环境下的自适应渐变底图
    const grad = bgCtx.createLinearGradient(0, 0, viewW, viewH);
    grad.addColorStop(0, "#0f172a");
    grad.addColorStop(1, "#1e293b");
    bgCtx.fillStyle = grad;
    bgCtx.fillRect(0, 0, viewW, viewH);

    // 绘制柔和测试网格
    bgCtx.strokeStyle = "rgba(56, 189, 248, 0.08)";
    bgCtx.lineWidth = 1;
    for (let x = 0; x < viewW; x += 40) {
      bgCtx.beginPath();
      bgCtx.moveTo(x, 0);
      bgCtx.lineTo(x, viewH);
      bgCtx.stroke();
    }
    for (let y = 0; y < viewH; y += 40) {
      bgCtx.beginPath();
      bgCtx.moveTo(0, y);
      bgCtx.lineTo(viewW, y);
      bgCtx.stroke();
    }
  }
}

// 绘制暗色半透明遮罩 (选区外部暗化，选区内部高亮透明)
function drawMask() {
  maskCtx.clearRect(0, 0, viewW, viewH);
  maskCtx.fillStyle = "rgba(0, 0, 0, 0.45)";
  maskCtx.fillRect(0, 0, viewW, viewH);

  if (currentSelection && currentSelection.width > 2 && currentSelection.height > 2) {
    maskCtx.clearRect(
      currentSelection.x,
      currentSelection.y,
      currentSelection.width,
      currentSelection.height
    );
  }
}

// 绘制涂鸦标注
function drawAnnotations() {
  drawCtx.clearRect(0, 0, viewW, viewH);

  const allStrokes = [...strokes];
  if (currentStroke) {
    allStrokes.push(currentStroke);
  }

  for (const s of allStrokes) {
    drawCtx.strokeStyle = s.color;
    drawCtx.fillStyle = s.color;
    drawCtx.lineWidth = 2.5;
    drawCtx.lineCap = "round";
    drawCtx.lineJoin = "round";

    if (s.tool === "pen" && s.points && s.points.length > 0) {
      drawCtx.beginPath();
      drawCtx.moveTo(s.points[0].x, s.points[0].y);
      for (let i = 1; i < s.points.length; i++) {
        drawCtx.lineTo(s.points[i].x, s.points[i].y);
      }
      drawCtx.stroke();
    } else if (s.tool === "rect" && s.start && s.end) {
      const rx = Math.min(s.start.x, s.end.x);
      const ry = Math.min(s.start.y, s.end.y);
      const rw = Math.abs(s.end.x - s.start.x);
      const rh = Math.abs(s.end.y - s.start.y);
      drawCtx.beginPath();
      drawCtx.strokeRect(rx, ry, rw, rh);
    } else if (s.tool === "arrow" && s.start && s.end) {
      drawArrow(drawCtx, s.start.x, s.start.y, s.end.x, s.end.y);
    }
  }
}

// 箭头绘制函数
function drawArrow(
  ctx: CanvasRenderingContext2D,
  fromX: number,
  fromY: number,
  toX: number,
  toY: number
) {
  const headLen = 12;
  const angle = Math.atan2(toY - fromY, toX - fromX);

  ctx.beginPath();
  ctx.moveTo(fromX, fromY);
  ctx.lineTo(toX, toY);
  ctx.stroke();

  // 箭头尖角两翼
  ctx.beginPath();
  ctx.moveTo(toX, toY);
  ctx.lineTo(
    toX - headLen * Math.cos(angle - Math.PI / 6),
    toY - headLen * Math.sin(angle - Math.PI / 6)
  );
  ctx.lineTo(
    toX - headLen * Math.cos(angle + Math.PI / 6),
    toY - headLen * Math.sin(angle + Math.PI / 6)
  );
  ctx.closePath();
  ctx.fill();
}

function redrawAll() {
  drawBackgroundImage();
  drawMask();
  drawAnnotations();
  updateSelectionBoxUI();
}

// 更新选区交互元素及 Focus-First 药丸胶囊位置
function updateSelectionBoxUI() {
  if (!currentSelection || currentSelection.width < 5 || currentSelection.height < 5) {
    selectionBox.style.display = "none";
    snipHint.style.display = "block";
    return;
  }

  snipHint.style.display = "none";
  selectionBox.style.display = "block";
  selectionBox.style.left = `${currentSelection.x}px`;
  selectionBox.style.top = `${currentSelection.y}px`;
  selectionBox.style.width = `${currentSelection.width}px`;
  selectionBox.style.height = `${currentSelection.height}px`;

  dimBadge.textContent = `${Math.round(currentSelection.width)} × ${Math.round(currentSelection.height)} px`;

  // 边界自适应判定：若选区贴近屏幕底边，药丸胶囊自动翻转至选区上方
  if (currentSelection.y + currentSelection.height + 60 > viewH) {
    focusPill.classList.add("docked-top");
  } else {
    focusPill.classList.remove("docked-top");
  }
}

// 初始化截屏数据载入
function loadCaptureData(capture: ScreenCaptureResult) {
  strokes = [];
  currentSelection = null;
  isDrawMode = false;
  drawDrawer.style.display = "none";
  btnToggleDraw.classList.remove("active");

  const img = new Image();
  img.onload = () => {
    bgImage = img;
    resizeCanvases();
  };
  img.src = capture.image_data_url;
}

// 事件监听：从 Rust 后端接收截屏帧
if (isTauri()) {
  listen<ScreenCaptureResult>("glint-snipping-capture", (event) => {
    loadCaptureData(event.payload);
  });
} else {
  // 独立浏览器调试演示兜底
  resizeCanvases();
}

window.addEventListener("resize", resizeCanvases);

// 鼠标交互：框选、拖动、调整尺寸、涂鸦
container.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;

  const target = e.target as HTMLElement;

  // 点击药丸按钮或抽屉内部组件，不触发框选
  if (target.closest(".focus-pill")) return;

  // 1. 判定是否点击了 8 方向尺寸手柄
  const handleEl = target.closest(".resize-handle") as HTMLElement;
  if (handleEl && currentSelection) {
    isResizing = true;
    activeResizeHandle = handleEl.dataset.handle || "";
    startPoint = { x: e.clientX, y: e.clientY };
    originalSelection = { ...currentSelection };
    e.stopPropagation();
    return;
  }

  // 2. 涂鸦模式开启时，在选区内优先执行涂鸦手绘
  if (isDrawMode && currentSelection) {
    const inSelection =
      e.clientX >= currentSelection.x &&
      e.clientX <= currentSelection.x + currentSelection.width &&
      e.clientY >= currentSelection.y &&
      e.clientY <= currentSelection.y + currentSelection.height;

    if (inSelection) {
      isDrawing = true;
      if (currentTool === "pen") {
        currentStroke = {
          tool: "pen",
          color: currentColor,
          points: [{ x: e.clientX, y: e.clientY }],
        };
      } else {
        currentStroke = {
          tool: currentTool,
          color: currentColor,
          start: { x: e.clientX, y: e.clientY },
          end: { x: e.clientX, y: e.clientY },
        };
      }
      return;
    }
  }

  // 3. 点击已有选区内部 (未开启涂鸦模式) -> 整体平移拖拽
  if (currentSelection && !isDrawMode) {
    const inSelection =
      e.clientX >= currentSelection.x &&
      e.clientX <= currentSelection.x + currentSelection.width &&
      e.clientY >= currentSelection.y &&
      e.clientY <= currentSelection.y + currentSelection.height;

    if (inSelection) {
      isMoving = true;
      moveOffset = {
        x: e.clientX - currentSelection.x,
        y: e.clientY - currentSelection.y,
      };
      return;
    }
  }

  // 4. 其余情况 -> 开始新的框选
  isSelecting = true;
  startPoint = { x: e.clientX, y: e.clientY };
  currentSelection = { x: e.clientX, y: e.clientY, width: 0, height: 0 };
  strokes = [];
  drawAnnotations();
  drawMask();
  updateSelectionBoxUI();
});

window.addEventListener("mousemove", (e) => {
  if (isSelecting) {
    const x = Math.min(startPoint.x, e.clientX);
    const y = Math.min(startPoint.y, e.clientY);
    const width = Math.abs(e.clientX - startPoint.x);
    const height = Math.abs(e.clientY - startPoint.y);
    currentSelection = { x, y, width, height };
    drawMask();
    updateSelectionBoxUI();
  } else if (isMoving && currentSelection) {
    let nx = e.clientX - moveOffset.x;
    let ny = e.clientY - moveOffset.y;
    nx = Math.max(0, Math.min(viewW - currentSelection.width, nx));
    ny = Math.max(0, Math.min(viewH - currentSelection.height, ny));
    currentSelection.x = nx;
    currentSelection.y = ny;
    drawMask();
    updateSelectionBoxUI();
  } else if (isResizing && currentSelection) {
    const dx = e.clientX - startPoint.x;
    const dy = e.clientY - startPoint.y;
    let { x, y, width, height } = originalSelection;

    if (activeResizeHandle.includes("w")) {
      const newW = width - dx;
      if (newW > 10) {
        x += dx;
        width = newW;
      }
    }
    if (activeResizeHandle.includes("e")) {
      const newW = width + dx;
      if (newW > 10) width = newW;
    }
    if (activeResizeHandle.includes("n")) {
      const newH = height - dy;
      if (newH > 10) {
        y += dy;
        height = newH;
      }
    }
    if (activeResizeHandle.includes("s")) {
      const newH = height + dy;
      if (newH > 10) height = newH;
    }

    currentSelection = { x, y, width, height };
    drawMask();
    updateSelectionBoxUI();
  } else if (isDrawing && currentStroke) {
    if (currentStroke.tool === "pen" && currentStroke.points) {
      currentStroke.points.push({ x: e.clientX, y: e.clientY });
    } else if (currentStroke.tool === "rect" || currentStroke.tool === "arrow") {
      currentStroke.end = { x: e.clientX, y: e.clientY };
    }
    drawAnnotations();
  }
});

window.addEventListener("mouseup", () => {
  if (isSelecting) {
    isSelecting = false;
    // 如果框选面积微小，判定为手滑单击，重置选区
    if (currentSelection && (currentSelection.width < 10 || currentSelection.height < 10)) {
      currentSelection = null;
      drawMask();
      updateSelectionBoxUI();
    }
  }

  isMoving = false;
  isResizing = false;

  if (isDrawing && currentStroke) {
    strokes.push(currentStroke);
    currentStroke = null;
    isDrawing = false;
    drawAnnotations();
  }
});

// Focus-First 药丸按钮交互
// 1. 展开/收起涂鸦标注抽屉
btnToggleDraw.addEventListener("click", (e) => {
  e.stopPropagation();
  isDrawMode = !isDrawMode;
  if (isDrawMode) {
    drawDrawer.style.display = "inline-flex";
    btnToggleDraw.classList.add("active");
  } else {
    drawDrawer.style.display = "none";
    btnToggleDraw.classList.remove("active");
  }
});

// 2. 工具切换
document.querySelectorAll(".tool-btn[data-tool]").forEach((btn) => {
  btn.addEventListener("click", (e) => {
    e.stopPropagation();
    document.querySelectorAll(".tool-btn[data-tool]").forEach((b) => b.classList.remove("active"));
    const toolBtn = btn as HTMLButtonElement;
    toolBtn.classList.add("active");
    currentTool = (toolBtn.dataset.tool as DrawTool) || "pen";
  });
});

// 3. 颜色切换
document.querySelectorAll(".color-dot").forEach((dot) => {
  dot.addEventListener("click", (e) => {
    e.stopPropagation();
    document.querySelectorAll(".color-dot").forEach((d) => d.classList.remove("active"));
    const dotEl = dot as HTMLElement;
    dotEl.classList.add("active");
    currentColor = dotEl.dataset.color || "#38bdf8";
  });
});

// 4. 撤销
btnUndoDraw.addEventListener("click", (e) => {
  e.stopPropagation();
  if (strokes.length > 0) {
    strokes.pop();
    drawAnnotations();
  }
});

// 5. 退出取消
btnCancel.addEventListener("click", async (e) => {
  e.stopPropagation();
  await cancelSnipping();
});

async function cancelSnipping() {
  currentSelection = null;
  strokes = [];
  redrawAll();

  if (isTauri()) {
    try {
      await invoke("close_snipping_tool");
    } catch (err) {
      console.error("关闭截屏视窗失败:", err);
    }
  }
}

// 6. 核心行动：合成裁剪 Base64 并提交多模态透视
async function submitSelection() {
  const sel = currentSelection || { x: 0, y: 0, width: viewW, height: viewH };
  if (sel.width < 5 || sel.height < 5) return;

  // 创建高精度离屏画布进行合成裁剪 (兼顾高 DPI 物理缩放)
  const cropCanvas = document.createElement("canvas");
  const cropW = Math.round(sel.width * dpr);
  const cropH = Math.round(sel.height * dpr);
  cropCanvas.width = cropW;
  cropCanvas.height = cropH;
  const cropCtx = cropCanvas.getContext("2d")!;

  // 1. 截取底图
  cropCtx.drawImage(
    bgCanvas,
    Math.round(sel.x * dpr),
    Math.round(sel.y * dpr),
    cropW,
    cropH,
    0,
    0,
    cropW,
    cropH
  );

  // 2. 叠加上层涂鸦标注
  cropCtx.drawImage(
    drawCanvas,
    Math.round(sel.x * dpr),
    Math.round(sel.y * dpr),
    cropW,
    cropH,
    0,
    0,
    cropW,
    cropH
  );

  const croppedDataUrl = cropCanvas.toDataURL("image/png");

  if (isTauri()) {
    try {
      await invoke("submit_snipping_result", {
        imageDataUrl: croppedDataUrl,
        prompt: "请分析此屏幕截图区域，直击关键信息与核心本质。",
      });
    } catch (err) {
      console.error("提交截屏结果失败:", err);
    }
  } else {
    console.log("浏览器预览模式提交截屏 Base64:", croppedDataUrl.slice(0, 80) + "...");
    alert("截屏合成成功！已捕获 " + sel.width + "x" + sel.height + " 选区与标注。");
  }

  currentSelection = null;
  strokes = [];
  redrawAll();
}

btnSubmitExplain.addEventListener("click", (e) => {
  e.stopPropagation();
  submitSelection();
});

// 全局键盘监听 (Esc 退出，Enter 提交，Ctrl+Z 撤销)
window.addEventListener("keydown", async (e) => {
  if (e.key === "Escape") {
    await cancelSnipping();
  } else if (e.key === "Enter") {
    await submitSelection();
  } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
    if (strokes.length > 0) {
      strokes.pop();
      drawAnnotations();
    }
  }
});
