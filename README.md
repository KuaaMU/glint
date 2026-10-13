<p align="center">
  <img src="public/app_icon_master.png" width="128" height="128" alt="Glint Icon" />
</p>

<h1 align="center">Glint · 微光</h1>

<p align="center">
  <strong>极简原生的伴生 AI 划词透视与阅读伴侣</strong><br/>
  <em>An ambient, featherweight AI reading companion built with Rust & Tauri 2.0.</em>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2021-DEA584?logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/Tauri-v2.0-24C8D8?logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/TypeScript-5.x-3178C6?logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Windows-Tier--1_Production-0078D6?logo=windows&logoColor=white" alt="Windows Tier-1" />
  <img src="https://img.shields.io/badge/License-MIT-green.svg" alt="License" />
</p>

---

## 🖥️ 平台支持矩阵 (Platform Support Matrix)

| 平台 (Platform) | 支持级别 (Tier) | 功能完备度说明 |
| :--- | :--- | :--- |
| **Windows 10 / 11** | 🟢 **主力生产 (Tier 1)** | 完整原生支持：Win32 零冲突剪贴板序列号感知 (`GetClipboardSequenceNumber`)、底层全局热键独立线程消息循环 (`RegisterHotKey`)、前台宿主应用感知 (`GetForegroundWindow`) 与多 DPI 自适应缩放。 |
| **macOS** | 🟡 **实验性预览桩 (Experimental Preview)** | 跨平台接口骨架搭建完毕；系统级辅助功能权限、全局热键事件监听与 AppleScript 桥接持续演进中。 |
| **Linux (X11 / Wayland)** | 🟡 **实验性预览桩 (Experimental Preview)** | 跨平台接口骨架搭建完毕；依赖 xdotool 采词适配，Wayland portal 全局快捷键机制演进中。 |

---

## ✨ 核心特性 (Key Features)

- ⚡ **毫秒级伴生卡片 (Ambient HUD Card)**  
  在任意应用（浏览器、PDF 阅读器、IDE、论文文献）中选中文本后，按下全局热键 `Alt + Q`，伴生 HUD 卡片即刻在光标旁轻盈浮现，流式输出深入浅出的概念剖析。
- 🛡️ **无冲突采词引擎 (Sentinel Probe)**  
  采用安全时序探测与系统剪贴板快照自动复原机制。若未选中文本，绝不覆盖用户原有剪贴板，彻底杜绝脏数据串台与旧内容幽灵残留。
- 🔒 **零依赖安全排版与 XSS 免疫 (Safe Markdown Pipeline)**  
  严格先行转义所有 HTML 实体，彻底防御任何 `<script>`、`<img onerror>`、`<svg>` 等 XSS 注入；支持流式代码块、行内代码、粗体排版；内置 3000 字符 Unicode 截断防御与优雅琥珀胶囊提示，绝不撑爆上下文。
- 📸 **截屏视觉透视 (Snipping Lens)**  
  快捷键 `Alt + W` 一键呼出全屏微光遮罩，1px 高清框选屏幕焦点区域，自动进行多模态视觉理解与多轮追问。
- 💬 **连续多轮追问 (Context-Aware Multi-turn)**  
  伴生卡片底部内置轻量级追问通道，支持针对专业名词或系统原理持续展开多轮深入探讨；划选新词条时独立 Session 瞬间重置。
- 🌐 **多模型与本地端点自适应 (LLM Agnostic)**  
  原生支持 **DeepSeek**（包括 DeepSeek-Reasoner 深度思考模型）、**OpenAI**、**Moonshot Kimi**、**Ollama (本地私有大模型)** 及任意 OpenAI 规范兼容端点。内置模型自动探测引擎，一键拉取端点支持的模型清单。
- 💎 **空间折光透镜视觉 (Spatial Refractive Lens)**  
  基于深空暗色（`#0B0E17`）与磨砂微发光质感打造，配合菲涅尔精密线稿图标。卡片边缘无死区微缩放手柄，支持 DPI 自适应与视窗记忆。
- ⚙️ **原生主线程渲染偏好设置 (Preferences Window)**  
  独立的后台设置视窗，支持 API Key 密码保护显隐、延迟探测、置顶开关与视窗尺寸一键重置；生命周期托管于 UI 主循环，零卡死、零白板。

---

## ⌨️ 快捷键指南 (Shortcuts)

| 快捷键 | 功能说明 |
| :--- | :--- |
| <kbd>Alt + Q</kbd> | **划词透视**：选中文本后一键触发伴生透视与深度解析 |
| <kbd>Alt + W</kbd> | **截屏透视**：全屏微光遮罩，1px 精准框选与多模态视觉解析 |
| <kbd>Alt + D</kbd> | **卡片显隐**：在当前光标位置快速呼出或隐藏伴生卡片 |
| <kbd>Esc</kbd> | **快速收起**：在卡片、截屏或偏好设置视窗中按 Esc 立即隐藏 |

---

## 🛠️ 本地开发与构建 (Getting Started)

### 1. 前置依赖 (Prerequisites)
- [Node.js](https://nodejs.org/) (建议 v18+) & [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/) (建议最新 stable toolchain)
- Windows: [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (Windows 10/11 通常已内置)

### 2. 克隆项目与安装依赖
```bash
git clone https://github.com/KuaaMU/glint.git
cd glint

# 安装前端依赖
pnpm install
```

### 3. 运行前端安全测试
```bash
node --experimental-strip-types src/safe-markdown.test.ts
```

### 4. 开发调试模式 (Dev Mode)
```bash
pnpm tauri dev
```

### 5. 生产包编译 (Release Build)
```bash
pnpm tauri build
```
编译产物位于 `src-tauri/target/release/glint.exe`。

---

## 📐 架构设计 (Architecture)

```
glint/
├── index.html              # 伴生卡片主视窗
├── preferences.html        # 偏好设置独立视窗
├── snipping.html           # 全屏截屏透视视窗
├── src/                    # 前端界面 (TypeScript + CSS)
│   ├── main.ts             # 伴生卡片 HUD 逻辑与流式渲染 (XSS 防御)
│   ├── safe-markdown.ts    # 零依赖安全转义、Markdown 解析与 3000 字截断
│   ├── safe-markdown.test.ts # 单元测试套件 (XSS 防御、边界截断、排版)
│   ├── preferences.ts      # 偏好设置交互、模型探测与配置管理
│   ├── snipping.ts         # Canvas 交互式选区截屏与多模态调度
│   ├── styles.css          # 卡片暗色半透明视觉与流光排版样式
│   └── preferences.css     # 偏好设置沉浸暗色面板样式
├── src-tauri/              # Rust 后端内核
│   ├── src/
│   │   ├── main.rs         # 应用启动入口
│   │   ├── lib.rs          # Tauri 运行时编排、托盘与主事件循环
│   │   ├── capture.rs      # Win32 剪贴板安全快照与序列号采词引擎
│   │   ├── llm.rs          # OpenAI / DeepSeek 多模态流式请求适配器
│   │   ├── preferences.rs  # 配置存取、模型端点探测与连接测速
│   │   ├── config.rs       # 本地配置持久化模型
│   │   └── platform/       # 平台原生底层适配模块
│   │       ├── mod.rs      # 跨平台分发抽象
│   │       ├── windows.rs  # Win32 原生全局热键、前台窗口感知与高 DPI 适配 (Tier 1)
│   │       ├── macos.rs    # macOS 原生适配桩 (Experimental)
│   │       └── linux.rs    # Linux 原生适配桩 (Experimental)
│   ├── icons/              # 菲涅尔空间折光透镜图标集
│   ├── Cargo.toml          # Rust 依赖清单
│   └── tauri.conf.json     # Tauri 视窗与应用元数据配置
└── public/                 # 静态资源与母版图标
```

---

## 📄 开源许可 (License)

本项目基于 [MIT License](LICENSE) 开源。欢迎 Star、Fork 与提交 Pull Request！
