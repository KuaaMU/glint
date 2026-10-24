# ADR 0012: Linux 原生伴生架构、KGlobalAccel 触发与 Slint 门禁

**状态:** Accepted
**日期:** 2026-10-19
**决策者:** User, System Architect

---

## 1. 背景与上下文 (Context)

Glint 从 Windows/Tauri 主线迁移到 Fedora 45 + KDE Plasma 6.7 + Wayland。迁移暴露了三类问题：

1. **架构文件与实现脱节**：ADR 0011 宣称已完成纯 Slint 双轨架构，但仓库中不存在根 `Cargo.toml`，不存在 `crates/glint-hud`，`crates/glint-core` 缺少 `config.rs` / `llm.rs` 且反向依赖 `tauri::AppHandle`，无法编译。
2. **验收文件失真**：`TEST_READY.md` 声称 115/115 全绿；实际运行 107/115，且多项“已验证”项只是读取 `PROJECT.md` 的静态字符串。Slint 特性、RAM < 15MB、延迟 < 20ms 均从未在运行时测量。
3. **Wayland 能力边界**：全局热键、窗口定位、置顶、模糊、截屏在 Wayland 下都不能沿用 X11/Win32 方案。直接复用 Tauri v1 的 global-shortcut、`mouse_position`、`set_position`、`screenshots` 会在 Wayland 失效。

### 1.1 外部实证

- **Pot（Tauri v1，GPL-3.0，已归档）**：选区读取本身是正确的——Linux 上通过 `selection` crate 读 PRIMARY，Wayland 分支使用 `wl-clipboard-rs`，不模拟 Ctrl+C、不污染剪贴板。它失效的是**触发与定位**：Tauri v1 全局快捷键仅 X11，`mouse_position` / `set_position` 在 Wayland 无效。其 HTTP `127.0.0.1:60828/selection_translate` 是实用的兜底触发器。
- **Manggo（闭源，Qt 6.9.3/QML，已在目标机安装）**：原生 Wayland 运行；`~/.config/kglobalshortcutsrc` 中存在 `[com.pylogmon.Manggo]` 组件与 `Alt+G/X/T/D` 默认键；二进制引用 `org.kde.kglobalaccel`、`org.freedesktop.portal.GlobalShortcuts`、`ext_data_control_*`、`wlr_layer_shell_unstable_v1`、`org.freedesktop.portal.Screenshot/RemoteDesktop`。它之所以在 Linux 上“快捷键都可用”，是因为使用了 KDE 原生接口，而不是 Tauri 的 X11 快捷键插件。
- **Slint 1.18**：`StyledText` 明确不支持 headings、tables、block quotes、images、math；fenced code block 无内建渲染；`StyledText` 无选中/复制 API；Wayland 下 `TextInput` >40 字符存在冻结 issue；`always_on_top` / `set_position` 在 Wayland 为 no-op；公开桌面应用内存约 25–50 MB，不支持 ADR 0011 的 <15 MB 断言。
- **Tauri v2 / WebKitGTK 2.54**：Wayland 窗口限制与 Slint 相同，但有 `gtk-layer-shell` 逃生通道；WebKitGTK 2.54 在 Plasma Wayland 上存在透明合成回归，需要目标机实测与不透明回退。

---

## 2. 决策 (Decision)

### 2.1 核心先行，UI 做薄壳

建立根 Cargo workspace。`crates/glint-core` 为 UI 无关的深模块，Tauri/Slint/CLI 只做适配层。平台差异收敛为 trait：`SelectionSensor`、`HotkeyEngine`、`ScreenGrabber`、`WindowPoser`。Windows 实现保留在 `cfg(windows)`。

### 2.2 UI 主线：Tauri 保留 6 个月，Slint 走门禁

- 未来 6 个月以 **Tauri v2 + 现有 TypeScript UI** 为可交付主线。
- 同时做一个 **2 周时间盒的 Slint HUD spike**（`crates/glint-hud`），只覆盖 HUD，不重写 preferences/snipping。
- 只有下列门禁全部通过，Slint 才成为 HUD 主壳；否则 ADR 0011 标记为 Superseded，附测量证据。

| 门禁 | 阈值 |
|---|---|
| Markdown 六类结构（标题/列表/粗体/行内码/代码块/表格） | ≤ 40 开发小时达到可用 |
| 中文 IME（fcitx5，200 字） | 0 丢字，preedit 跟随光标，按键延迟 < 50 ms |
| 流式 2000 token 渲染 | p95 帧时间 ≤ 16 ms，无全量重排 |
| 空闲 PSS（10 分钟后） | ≤ 40 MB |
| 冷启动到首帧 | ≤ 150 ms |
| 置顶/定位 | 有可复现的 KWin 规则或 layer-shell 方案 |

### 2.3 全局热键：KDE 上 KGlobalAccel 优先

```
XDG_CURRENT_DESKTOP 含 KDE  → org.kde.KGlobalAccel D-Bus（默认键 + 系统设置持久化）
其他 Wayland 桌面           → org.freedesktop.portal.GlobalShortcuts
X11 会话                    → XGrabKey
```

默认键避开 Manggo 已占用的 `Alt+D`：`Alt+Q` 划词、`Alt+W` 截屏、`Alt+Shift+D` 显隐。全部可配置，启动时检测冲突。另提供 `glint-trigger` CLI（仅回环 Unix socket 或 localhost）作为通用兜底，用户可在 KDE 系统设置中绑定。

### 2.4 选区读取：PRIMARY，先读后显

- Wayland：`wl-clipboard-rs 0.9.4` 读 `ClipboardType::Primary`（`ext_data_control` / `zwp_primary_selection`）。
- X11：`x11rb` 读 PRIMARY。
- AT-SPI `TextProxy` 仅作补充；Chromium/终端常不暴露。
- **不合成 Ctrl+C**；确需按键回写时走 RemoteDesktop/libei + `reis`。
- 当前实现（`src-tauri/src/lib.rs::capture_current_selection`）：Wayland 会话先读 PRIMARY，失败才回退到 XWayland 的 Ctrl+C；X11 会话反过来。原因是原生 Wayland 窗口收不到 X 合成按键，此时 Ctrl+C 可能落到上一个仍有 X 焦点的窗口，读到无关文本。X11 回退需要 `xdotool`，缺失时只走 PRIMARY。
- 读取顺序：先捕获选区，再显示/聚焦 HUD。
- 读取超时 150 ms，原始上限 64 KB，再做 3000 字符 Unicode 截断。
- 失败返回明确的 `CaptureError`，不显示上一轮旧文本。
- v0.3 增加短 TTL 的选区候选缓冲（订阅 `ext-data-control`），降低热键与焦点变化之间的竞态。

### 2.5 窗口：layer-shell 为主，XWayland 兜底

- 使用 `zwlr_layer_shell_v1`（`gtk-layer-shell` 或小型 native overlay helper）创建 overlay 层表面。
- v1 采用**指针所在输出的边缘/角落锚定**；Wayland 不允许查询全局光标位置，不承诺光标旁精确跟随。
- 可选增强：KWin 脚本读取 `workspace.cursorPos`，用 layer-shell margin 做光标旁定位。
- 置顶、跳过任务栏、点击穿透均为 compositor 侧能力：发布按 `resourceClass` 匹配的 KWin 脚本/窗口规则；点击穿透使用空 input region。
- 模糊使用 Plasma 6.7 的 `ext_background_effect_manager_v1`；默认设计必须有“不透明/半透明 + 1px hairline”的降级形态。
- 若 layer-shell 或 WebKitGTK 2.54 透明合成不可用，HUD 强制走 XWayland（`GDK_BACKEND=x11`），热键仍走 KGlobalAccel/portal。

### 2.6 截屏与上下文

- 静图：`org.freedesktop.portal.Screenshot`。
- 连续捕获（后续）：`ScreenCast` + PipeWire + restore token。
- 活动窗口标题/类别：KWin 脚本或 `kdotool`，没有非交互式 D-Bus getter。
- 剪贴板历史：Klipper D-Bus 仅作补充，不作为主路径。

### 2.7 Agent runtime：单次流式 + 有界工具循环

- 默认单次流式完成；最多 **2 轮**工具循环。
- 3 个本地工具：`lookup_doc`、`calc_sandbox`、`system_context`。
- 按 `index` 累积 SSE `tool_calls`，等 `finish_reason == "tool_calls"` 再执行；结果以 `role:"tool"` 回填后二次请求。
- 不引入 planner/executor、多 agent、MCP client 市场；后续可将 Glint 暴露为 MCP server。
- 删除/替换“假装是模型”的离线硬编码回答；离线必须是明确的离线或缓存状态。

### 2.8 上下文与输出

- `ContextEnvelope`：选区、段落、宿主 app/窗口标题、会话轮次、深度偏好、时间。
- 缓存前缀顺序：固定 system（角色/输出契约/widget schema）→ 会话级 → 每轮。`window_title` 不再拼进 system prompt 首位。
- 预算：system 700 + envelope 800 + history 1500 + output 700 ≈ 3.7K tokens；超过 6 轮生成 150-token session brief。
- 存储：SQLite FTS5 + 精确匹配 LRU；10K 条以内不使用向量库；v1 不做语义缓存。
- 密钥：KWallet / Secret Service；发送前做密钥/邮箱/电话/高熵串脱敏。
- 渲染：块级增量、每帧 flush、未闭合块按纯文本；代码块闭合后高亮。
- widget 使用结构化 JSON envelope（`answer_md` + 可选 `widget`），不从流式文本抓 ```widget:*。

### 2.9 视觉与交互

- 深色默认 + 浅色一套；保留青蓝 `#38bdf8` 品牌色，仅用于主操作/焦点/引用标记（≤5% 面积）。
- 字体：Inter / Noto Sans SC；等宽 JetBrains Mono。UI 13/18，正文 14/21。
- 间距 4 基数；圆角 16/10/8；HUD 未固定 560–640 px，固定后 680×520。
- 动效：进入 120–180 ms，退出 90–120 ms；只让新 token 做 120 ms blur-in；不使用固定节奏打字机；`AnimationDurationFactor=0` 时全部关闭。
- P0 交互：选区 8 px 偏移、上下翻转、Esc 任意焦点可退、Ctrl/Cmd+Enter 次动作、Stop 随时可点、按指针所在屏幕。
- 信任：输入框旁常驻本地/云端路由 chip；API Key 存储位置可见。

### 2.10 验收：契约测试与运行时验收分离

- 契约测试不得用 `PROJECT.md` 字符串替代实现；目标不存在时标记 `skip`，不得报 pass。
- 运行时验收按 `tests/RUNTIME_ACCEPTANCE.md` 在真实会话执行。
- 禁止在测试汇总中无条件打印“已验证”。

---

## 3. 方案对比 (Options Considered)

| 维度 | A. Tauri + Web（选定） | B. Slint 纯原生（门禁） | C. Qt/QML（Manggo 路线） | D. 本地 HTTP 后端 + UI |
|---|---|---|---|---|
| 富文本/流式 Markdown | 现成 | 需自建，缺标题/代码块/表格 | 强 | 强 |
| 中文 IME | 可用（有已知 caret 位置问题） | 用户报告可用，但有大输入冻结 issue | 成熟 | 成熟 |
| Wayland 窗口控制 | layer-shell 可接 | 无 layer-shell，需自写 surface | Qt 原生支持好 | 取决于 UI |
| 内存 | 84–125 MB PSS | 25–50 MB | 中等 | 更高 |
| 迁移成本 | 0 | 约 6,000 行 UI 重写 | 全新重写 | 新增 IPC/鉴权层 |
| 生态与开发速度 | 高 | 中 | 高（但闭源参考） | 低 |

**否决 D**：桌面伴生工具引入本地 HTTP 服务只增加进程、端口、鉴权和攻击面，不解决任何当前问题。`glint-trigger` 仅作为单命令触发器，不是业务后端。

---

## 4. 后果 (Consequences)

### 更容易
- 核心逻辑可在 Tauri/Slint/CLI 之间复用，Slint 决策不再阻塞 Linux 可用性。
- KGlobalAccel + PRIMARY + layer-shell 的组合与目标机既有产品（Manggo）行为一致，风险可测。
- 契约与运行时验收分离，后续不会再用静态字符串冒充交付。

### 更困难
- 需要维护 Tauri 与 Slint 两套窗口/事件适配（限 spike 期间）。
- layer-shell/点击穿透/KWin 脚本存在 4 个未验证点，必须先做 1 小时 spike。
- WebKitGTK 2.54 在 Plasma Wayland 的透明合成需要实机验证与降级设计。

### 需要复查
- 6 个月后 Slint 门禁结果；若通过，Tauri 退居何处。
- Plasma 6.8 移除 X11 会话后，XWayland 兜底是否仍可用（当前仍保留）。
- WebKitGTK 后续版本是否修复透明/IME caret 问题。

---

## 5. 行动项 (Action Items)

- [x] P0：根 workspace；`glint-core` 去 Tauri 化并通过 `cargo test -p glint-core`（15 passed）。
- [x] P0：`.gitattributes` 统一 LF；清理 233 行换行噪音。
- [x] P0：契约测试支持 `skip`；M2/占位测试不再假通过；`TEST_READY.md` 改为真实状态。
- [x] P0：安装 Fedora GTK/WebKit/DBus 开发头文件；`cargo check --workspace` 通过（2026-10-21）。
- [ ] P1：`SelectionSensor`（wl-clipboard-rs 0.9.4 + x11rb）、`HotkeyEngine`（KGlobalAccel/portal）、`WindowPoser`（layer-shell/XWayland）与 CLI harness。
- [ ] P1：把显隐默认键从 `Alt+D` 切换为 `Alt+Shift+D`（Manggo 已占用 `Alt+D`），并让 Windows 注册与配置同源。
- [ ] P1：4 个 1 小时 spike（portal host-app 注册、空 input region、`workspace.cursorPos`、Tauri ignore-cursor-events）。
- [ ] P2：Tauri HUD 接 layer-shell；流式/追问/偏好设置；Linux v0.2 运行时验收。
- [ ] P3：Slint HUD 门禁 spike。
- [ ] P4：有界工具循环、ContextEnvelope、SQLite 缓存、diff/持久 thread。

---

## 6. 证据来源 (Evidence)

- Slint `StyledText` 文档与限制：https://docs.slint.dev/latest/docs/slint/reference/elements/styledtext/
- Slint IME 输入冻结 issue：https://github.com/slint-ui/slint/issues/10912
- Slint Wayland 置顶/定位限制：https://github.com/slint-ui/slint/discussions/5261 · https://github.com/slint-ui/slint/issues/3032
- Pot `selection` crate Linux 实现：https://docs.rs/crate/selection/1.2.0/source/src/linux.rs
- Pot Wayland 文档：https://pot-app.com/docs/wayland.html
- xdg-desktop-portal GlobalShortcuts：https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html
- KDE GlobalShortcuts 后端：https://invent.kde.org/plasma/xdg-desktop-portal-kde/-/blob/Plasma/6.7/src/globalshortcuts.cpp
- wl-clipboard-rs：https://docs.rs/wl-clipboard-rs/latest/wl_clipboard_rs/
- winit Wayland `WindowLevel` 限制：https://docs.rs/winit/latest/winit/window/enum.WindowLevel.html
- Tauri Wayland `set_position` 限制：https://github.com/tauri-apps/tauri/issues/14913
- WebKitGTK 2.54 变更：https://webkitgtk.org/2026/09/16/webkitgtk-2.54-highlights.html
- Anthropic Building Effective Agents：https://www.anthropic.com/engineering/building-effective-agents
- Anthropic prompt caching：https://platform.claude.com/docs/en/build-with-claude/prompt-caching
