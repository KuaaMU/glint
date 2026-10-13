# ADR 0011: 纯原生 Slint 硬件加速视窗与深模块内核解耦架构

**状态:** Accepted  
**日期:** 2026-10-13  
**决策者:** System Architect, Core Engineer, Performance Specialist  

---

## 1. 背景与上下文 (Context)

在先前版本中，Glint 采用了基于 Tauri 2（Rust 后端 + WebView2 前端）的混合桌面架构。在实际运行中，该架构暴露出以下违背“深模块（Deep Module）”设计哲学与轻量桌面体验的深层缺陷：

### 1.1 严重的信息泄漏 (Severe Information Leakage)
- **表象**: 当开发端口或静态资源因打包命令错位时，WebView2 发生了 `ERR_CONNECTION_REFUSED`，直接向最终用户弹出了 Chromium/Edge 默认的“无法访问网页”报错页。
- **哲学反思**: 桌面应用本质应当是自封闭的宿主伴生程序，而 WebView2 将底层的网络连接、HTTP 端口与 Web 运行时的脆弱性直接泄漏给了视觉层，严重破坏了“原生伴生透视”的用户心理模型。

### 1.2 浅模块碎片化与多层 IPC 损耗 (Shallow Modules & IPC Friction)
- 前端 JS/TS 与 Rust 后端之间需要维持庞大的 IPC 序列化通道（JSON 序列化、异步 Channel、事件总线、窗口位置同步）；
- 为了实现一个平滑的窗口拉伸手柄，需要穿越“JS 鼠标事件 -> 跨进程 IPC -> Win32 消息循环 -> DWM 合成 -> WebView2 回流”多层浅模块堆叠，极易引发 IPC 饥饿与卡死；
- 常驻内存高达 40MB~70MB，冷启动耗时数百毫秒。

---

## 2. 核心架构决策 (Decisions)

根据 John Ousterhout《软件设计哲学》（*A Philosophy of Software Design*）的**“深模块（Deep Module: 极窄接口，极深封装，杜绝实现细节泄露）”**原则，彻底剥离 WebView2，全面转向 **100% 纯 Rust + Slint 原生 GUI 架构**。

```
                        ┌──────────────────────────────────────────────┐
                        │              Glint Workspace                 │
                        └───────┬──────────────────────────────┬───────┘
                                │                              │
                ┌───────────────▼──────────────┐ ┌─────────────▼────────────────┐
                │       crates/glint-core      │ │      crates/glint-hud        │
                │     (纯 Rust 共享深模块)     │ │   (100% 纯原生 Slint HUD)    │
                └───────────────┬──────────────┘ └─────────────┬────────────────┘
                                │                              │
                                └──────────────┬───────────────┘
                                               │
                        ┌──────────────────────▼───────────────────────┐
                        │            底层原生能力与向量渲染            │
                        │  - Slint 1.18.1 (FemtoVG / OpenGL 硬件加速)  │
                        │  - Win32 原生亚克力模糊 / 无边框透明窗口     │
                        │  - Win32 零冲突剪贴板序列号引擎 (内核态轮询) │
                        │  - 前台激活宿主窗口标题感知 (Capsule 胶囊)   │
                        │  - SSE 流式打字机管道 (直通 Slint 属性)      │
                        │  - 内存占用严格 < 15MB，冷启动 < 20ms        │
                        └──────────────────────────────────────────────┘
```

### 2.1 深模块 1：`crates/glint-core` (纯 Rust 认知与系统感知引擎)
- **极窄公共接口 (Narrow Interface)**:
  ```rust
  pub struct SelectionContext {
      pub text: String,
      pub cursor_x: i32,
      pub cursor_y: i32,
      pub window_title: Option<String>,
  }

  pub fn capture_context() -> Option<SelectionContext>;
  pub async fn stream_insight(req: QueryRequest, sink: impl FnMut(StreamToken));
  pub fn spawn_global_hotkeys(on_trigger: impl Fn(SelectionContext) + Send + 'static);
  ```
- **极深内聚封装 (Deep Implementation)**:
  - 隐藏 Win32 `GetClipboardSequenceNumber`、按键合成（Ctrl+C）与保护现场逻辑；
  - 隐藏前台 `GetForegroundWindow` / `GetWindowTextW` 窗口标题探测；
  - 隐藏 SSE 流式 HTTP/1.1 长连接、流式 Token 解包与本地沙箱执行。
  - **零外部 UI 依赖**：纯 Rust 标准库与网络栈，物理上不包含任何 WebView、HTML 或 DOM 概念。

### 2.2 深模块 2：`crates/glint-hud` (Slint 向量硬件加速极简 HUD)
- **极窄接口**:
  对外只提供 `run_hud_loop()` 与状态驱动接口。
- **深层实现**:
  - 采用 Slint 1.18.1 原生编译，基于 **FemtoVG / OpenGL** 纯硬件加速向量光栅化；
  - Win32 窗口透明通道配合原生 DWM 亚克力磨砂（Acrylic / Mica）；
  - 1px 流光渐变刃角与 0.5px 精细线稿（Hairline）；
  - 流式打字机字符原位渲染，交互式深入追问输入框；
  - 物理杜绝“网页加载”、“浏览器内核”、“网络断开白屏”等一切 Web 伪原生态异味。

---

## 3. 方案对比与权衡 (Trade-off Analysis)

| 评估维度 | 方案 A：Tauri 2 + WebView2 (旧主线) | 方案 B：Slint + 纯 Rust (本决策) |
| :--- | :--- | :--- |
| **模块深度** | 浅（跨进程 IPC、DOM、CSS 碎片化拼装） | **极深（纯 Rust 类型系统与原生函数直调）** |
| **信息泄漏度** | 严重（异常时直接暴露浏览器“无法访问网页”） | **零泄漏（物理级无浏览器、无网页协议）** |
| **常驻内存** | 40MB ~ 70MB (依赖 WebView2 渲染进程) | **< 15MB (单进程轻量轻盈)** |
| **冷启动延迟** | 300ms ~ 800ms | **< 20ms** |
| **拉伸缩放性能** | 需额外 OS 接口中转防阻塞 | **144Hz 满帧 GPU 向量即时重绘** |
| **富文本灵活性** | 高 (HTML/CSS 现成生态) | 需在 Slint 内自建优雅排版组件 (已有规划) |

---

## 4. 实施与验收标准 (Consequences & Action Items)

1. [x] 确立 Cargo 根工作区配置（`Cargo.toml` 统一管理 `crates/glint-core` 与 `crates/glint-hud`）；
2. [ ] 完成 `crates/glint-core` 纯 Rust 内核解耦与单元测试隔离；
3. [ ] 编写 `hud.slint` 向量美学组件（Obsidian 晶透亚克力、流光线稿、宿主胶囊、打字机输出）；
4. [ ] 编译生成独立原生可执行文件 `glint-hud.exe`，验证常驻内存 < 15MB 与冷启动 < 20ms。
