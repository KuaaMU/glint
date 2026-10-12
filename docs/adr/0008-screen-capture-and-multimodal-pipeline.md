# ADR 0008: 跨平台原生截屏引擎技术选型、多显示器 DPI 适配与 Focus-First 多模态透视架构

**状态:** Accepted  
**日期:** 2026-10-12  
**决策者:** System Architect, Core Systems Engineer, Frontend Design Lead  

---

## 1. 背景与上下文 (Context)

在 Glint 的下一阶段形态进化中，用户提出了具备“超前体验与极致丝滑”的**屏幕多模态透视助手**演进诉求（类似 PixPin 的轻盈截屏涂鸦标注体验，配合多模态大模型的视觉解析能力）。

实现“像素级瞬发、零卡顿、无模糊”的截屏与涂鸦体验，核心技术挑战集中在：
1. **跨平台原生截屏引擎的性能与兼容性**（尤其在 Windows 下各类图形 API 的延迟差异与多显卡兼容性）。
2. **多显示器混合 DPI 缩放与虚拟桌面几何坐标映射**（例如主屏 4K 175% DPI + 副屏 1080p 100% DPI 下的坐标偏位与位图模糊）。
3. **截屏选区交互与大模型推理链路的低侵扰融合**（如何在不打扰用户的前提下，实现选区、渐进涂鸦与一键多模态推理）。

---

## 2. Windows 平台原生截屏深度选型对比

在 Windows 10/11 体系下，存在三种主流截屏方案：

| 评估维度 | 方案 A: DXGI Desktop Duplication (DDA) | 方案 B: GDI BitBlt + 虚拟桌面全景 | 方案 C: Windows.Graphics.Capture (WGC) |
| :--- | :--- | :--- | :--- |
| **底层接口** | DirectX 11 / DXGI 1.2+ (`DuplicateOutput`) | Win32 GDI (`CreateCompatibleDC` + `BitBlt`) | WinRT / UWP 现代 API (`GraphicsCaptureItem`) |
| **首帧单次截屏延迟** | **3 ~ 8 ms** (GPU 显存显式映射) | **15 ~ 35 ms** (CPU 内存回读) | 20 ~ 50 ms (COM 代理与会话协商) |
| **多显卡/混合架构支持** | 较复杂（双显卡笔记本核显/独显跨输出需枚举多个适配器） | **100% 极高（由 DWM 统一合成输出）** | 良好（系统底层封装） |
| **驱动与权限异常恢复** | 偶发 `DXGI_ERROR_ACCESS_LOST`，需重新初始化流水线 | 永不报错，零崩溃，极佳容错 | 稳定，但后台进程需特定窗口上下文 |
| **光标合成 (Cursor)** | 需手动计算硬件鼠标指针位图并绘制到纹理 | 可通过 `DrawIconEx` 轻松按需合成光标 | 支持原生带光标与排除边框配置 |
| **系统版本兼容性** | Windows 8+ | **全系 Windows (Win7 / 10 / 11 / Server)** | 仅限 Windows 10 1803+ |

### 架构权衡结论
* 单独使用 DXGI 虽然在连续流录屏上占优，但在**单次瞬间截屏**场景中，双显卡切换、休眠唤醒、锁屏或全屏独占游戏时容易触发 DDA 访问拒绝。
* **Glint 采用“双模智能自适应混合流水线” (Hybrid Screen Capture Engine)**：
  1. 优先尝试 DXGI 极速捕获（显存级 5ms 瞬发）。
  2. 捕获失败或多适配器异常时，毫秒级无缝自动降级至 GDI 虚拟桌面全景捕获。
  3. 保障用户在任何硬件配置（包括核显轻薄本、外接多屏工作站）下都能 100% 稳定出图。

---

## 3. 多显示器 DPI 缩放与多屏拼接关键技术方案

### 3.1 DPI 意识模式锁定 (Per-Monitor DPI Aware V2)
若进程未声明最高 DPI 级别，Windows DWM 将对位图实施虚拟插值拉伸（DPI Virtualization），导致 4K 高分屏截屏字体发虚且坐标严重偏移。
* **决策**: Glint 在应用程序清单与 Rust 启动时显式调用：
  ```rust
  SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  ```

### 3.2 虚拟桌面全局几何映射 (Virtual Screen Geometry)
多显示器环境下，各显示器的逻辑坐标可能存在负数（例如主屏居中，左侧副屏 X 为负数）。
* **抓取范围定位**:
  - `x = GetSystemMetrics(SM_XVIRTUALSCREEN)`
  - `y = GetSystemMetrics(SM_YVIRTUALSCREEN)`
  - `w = GetSystemMetrics(SM_CXVIRTUALSCREEN)`
  - `h = GetSystemMetrics(SM_CYVIRTUALSCREEN)`
* GDI 创建对应整屏物理尺寸的内存设备上下文（Memory DC），单次 `BitBlt` 捕获整张虚拟桌面大画布，将所有屏幕在同一毫秒内锁定为静态快照，杜绝跨屏选区撕裂。

---

## 4. macOS 与 Linux 跨平台原生抽象

保持 Glint 核心的跨平台架构一致性（`cfg(target_os)`）：
* **macOS**:
  - macOS 12.3+：优先调用 `ScreenCaptureKit` 获取无锁帧。
  - macOS 10.15~12.2 兼容兜底：调用 CoreGraphics `CGWindowListCreateImage`（延迟约 10~18ms），原生支持 Retina 2x/3x 点阵到像素转换。
* **Linux**:
  - X11 环境：调用 `XGetImage` / `XShmGetImage` 共享内存扩展，直接读取根窗口（Root Window）。
  - Wayland 环境：基于 DBus 桥接 `org.freedesktop.portal.Screenshot` 协议规范，安全获取系统合成器快照。

---

## 5. 前端交互体系：Focus-First 渐进式极简透视 (Focus-First Lens UI)

基于 `/frontend-design` 与用户抉择，摒弃传统截图软件杂乱铺满屏幕的几十个图标工具栏，采用**“渐进披露 (Progressive Disclosure)”**设计语言：

```
+--------------------------------------------------------+
|                                                        |
|             [ 用户鼠标拉框的重点内容选区 ]              |
|                                                        |
+--------------------------------------------------------+
                                \
                                 \-- [ ✨ 透视 (Enter) ] [ ✎ 涂鸦 ]  <-- 默认极简微胶囊
                                            |
                                            +-- 点击展开 --> [ 矩形 ▢ | 箭头 ↗ | 画笔 ✎ | 撤销 ↺ ]
```

1. **默认态 (Focus-First)**:
   - 拉选区域后，选区右下角仅贴附包含 2 个核心药丸按钮的毛玻璃胶囊：
     - `✨ 透视 (Enter)`：直接把选区图像编码并送入 HUD 卡片流式推理。
     - `✎ 涂鸦`：点击后平滑展开微型画笔工具条。
2. **交互快捷通道**:
   - <kbd>Enter</kbd>：立即完成截屏并流式透视。
   - <kbd>Esc</kbd>：平滑淡出退出遮罩，不残留任何后台负荷。
   - <kbd>Ctrl + C</kbd>：仅复制截屏位图到系统剪贴板。

---

## 6. 多模态大模型对接协议规范 (Multimodal Protocol)

遵循 OpenAI 统一多模态规范，无缝兼容 `gpt-4o`、`qwen2.5-vl`、`deepseek-vl`、`ollama`：

```json
{
  "model": "gpt-4o",
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "请分析图中框选与标注的重点内容，用直白透彻的语言解释其核心逻辑、解决什么问题或底层原理："
        },
        {
          "type": "image_url",
          "image_url": {
            "url": "data:image/png;base64,iVBORw0KGgo..."
          }
        }
      ]
    }
  ],
  "stream": true
}
```

---

## 7. 后续 TDD 纵向切片实施计划 (TDD Execution Plan)

按照 TDD 红-绿循环规则划分四个严格独立的纵向切片：
1. **Slice 1: 多模态请求序列化与 SSE 流式解析 Seam (`src-tauri/src/llm.rs`)**
   - 编写单元测试：验证 `QueryRequest` 支持纯文本与 `image_url` 混合 payload 序列化，以及错误响应捕获。
2. **Slice 2: Windows 虚拟桌面 GDI/DXGI 截屏与内存位图编码 Seam (`src-tauri/src/capture.rs`)**
   - 编写单元测试：验证虚拟桌面多屏坐标计算、Bitmap 头信息校验、PNG 内存压缩流转换。
3. **Slice 3: Focus-First 前端 Canvas 选区与标注遮罩视窗 (`src/snipping.ts`)**
   - 编写状态测试：验证坐标拖拽换算、高 DPI 像素比缩放校正、撤销栈（Undo Stack）。
4. **Slice 4: 全局热键与窗口流转闭环**
   - 注册截图全局快捷键（如 `Alt + W`），打通“截屏遮罩 → 涂鸦 → Enter → HUD 伴生卡片流式解析”全流程。
