# ADR 0007: 零锁剪贴板序列采词流水线、动态模型发现与空间折光体视觉体系

**状态:** Accepted  
**日期:** 2026-10-11  
**决策者:** System Architect, Frontend Design Lead, Core Engineer  

---

## 1. 背景与上下文 (Context)

在上一阶段的真实物理机实测中，用户与测试反馈暴露了 3 个关键阻塞性问题：
1. **浏览器划词采词失败**: 划选文本后按下 `Alt + Q`，窗口虽唤出但捕获为空（"💡 未检测到选中文本"），无法自动进入解析会话。经系统深潜排查，根因在于：
   - 快捷键 `Alt + Q` 触发为按下事件（KeyDown）。用户手指物理抬起通常需 80~250ms；若在此期间立即模拟按键，目标进程（如 Chromium 内核）会判定为 `Ctrl + Alt + C`，拒绝执行复制。
   - 旧版使用哨兵探针覆盖剪贴板并以 20ms 频次密集调用 `OpenClipboard` 轮询，造成与目标渲染进程的 `CLIPBRD_E_CANT_OPEN` 互斥死锁。
2. **偏好设置视窗卡死与无法滚动**: 偏好设置页面加载了包含 `overflow: hidden; height: 100%` 的主视窗样式，造成纵向滚轮被锁死；同时缺少针对 `/v1/models` 的动态检索，用户输入 API 凭据后无法直观选择可用模型。
3. **视觉符号辨识度不足**: 旧图标方案 C 采用简化的二维折叠色块，缺乏产品“Glint（微光/透镜折光）”的核心隐喻，且在小尺寸下对比度不足。

---

## 2. 核心架构决策 (Decisions)

### 2.1 零锁剪贴板序列流水线 (Zero-Lock Sequence Capture Pipeline)

废除破坏性写入剪贴板的哨兵探针机制，采用 Windows 硬件级 `GetClipboardSequenceNumber` 与物理按键释放探测：
1. **物理按键释放仲裁**: 在调用模拟复制前，通过 `GetAsyncKeyState` 轮询等待用户手指真正松开 `Alt` 键（或超时 300ms）。
2. **全修饰键安全复位**: 合成所有修饰键（Left/Right Alt, Ctrl, Shift）的 `KEYUP` 状态并辅以 25ms 消息队列排空，随后注入携带硬件扫描码（`0x1D` 与 `0x2E`）的 `Ctrl + C`。
3. **内核级序列号无锁监听**: 记录 `initial_seq = GetClipboardSequenceNumber()`，通过 15ms 间隔轮询该数值变化。由于目标应用成功写入剪贴板必定触发内核序列号自增，此过程**完全无需打开或锁定剪贴板句柄**，彻底消除了访问冲突；未选中文本时用户原有剪贴板 100% 毫发无损。

### 2.2 双规模型动态拉取协议 (Dynamic Model Discovery Protocol)

在 Rust 端引入 `fetch_models` 命令，实现标准化自适应发现：
1. 优先探测标准 OpenAI 规范端点 `${baseUrl}/models`，解析 `.data[].id`。
2. 自动降级探测 Ollama 本地规范端点 `${baseUrl}/api/tags`，解析 `.models[].name`。
3. 智能权重重排：对包含 `chat`、`reasoner`、`gpt-4`、`qwen` 的对话类主力模型实施置顶高亮，去重后回传。
4. 前端提供 `<select>` 下拉点选与 `<datalist>` 自动补全双模交互，并在 URL / Key 失焦时执行温和静默预取。

### 2.3 空间折光体透镜视觉体系 (Folded Prism Lens Aesthetic System)

将视觉符号定位于 **“空间折光体透镜 (Folded Prism Lens)”**：
1. **多维轴测折面 (Isometric Planar Facets)**: 采用 6 组主维度空间折叠晶体面，结合折角环境光遮蔽（AO 阴影）与山脊白光刃线（Chine Highlights），呈现深邃的工业科技折纸感。
2. **微光聚焦晶核 (The "Glint" Focal Spark)**: 在折面中央汇聚点嵌入由白热四芒微光针核组成的聚焦耀斑，象征将繁杂黑话一针见血折射为通透本质。
3. **16px 托盘网格对齐**: 精准对齐 16x16 像素栅格，以高反差青紫双翼折面与纯白中心折线确保在浅色/深色系统栏下的穿透力。

---

## 3. 架构对比与收益 (Trade-offs)

| 维度 | 旧版哨兵轮询机制 | 新版零锁序列号流水线 |
| :--- | :--- | :--- |
| **按键时序鲁棒性** | 立即发射，易与物理 Alt 发生冲突 | 物理释放智能判定 + 扫描码注入，100% 消除误判 |
| **剪贴板锁争用** | 每 20ms 强行打开剪贴板，易致 Chrome 访问拒绝 | 仅读取 Win32 内核序列号，零句柄占用，无锁零冲突 |
| **用户数据安全性** | 覆写哨兵探针，存在丢失富文本格式风险 | 探针不写入剪贴板，未选中时原有剪贴板 100% 无损 |
| **模型配置体验** | 仅支持手工文本输入，易打错标识符 | 自动拉取端点清单，下拉直选，实时网络测延 |

---

## 4. 后续演进 (Consequences)

1. 在 macOS / Linux 平台中，继续通过 `cfg(target_os)` 桥接对应的 Cocoa `changeCount` 与 X11 `PRIMARY` Selection。
2. 继续支持用户在偏好设置中一键自由切换其他视觉备选方案（A: Prism Spark, B: Aperture Lens），满足个性化审美诉求。
