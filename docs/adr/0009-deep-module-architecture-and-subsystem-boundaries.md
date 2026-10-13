# ADR 0009: 深模块子系统边界解耦、增量式流式渲染与全链路工具调用引擎

**状态:** Accepted  
**日期:** 2026-10-13  
**决策者:** System Architect, Core Engineer, Frontend Lead  

---

## 1. 背景与上下文 (Context)

在引入“生成式 UI”、“Tool Call 工具调用”与“会话历史漫游”后，经过代码审查（Code Review）与架构复盘，系统暴露出若干典型的**“浅模块（Shallow Modules）”与“信息泄漏（Information Leakage）”**设计异味：

1. **前端主控过度膨胀 (God File in `src/main.ts`)**:
   `src/main.ts` 已突破 700 行，混杂了 Win32 视窗拖拽、原生 IPC 管道监听、流式 Markdown 实体拼接、Tool Call 胶囊动态插拔、会话历史本地持久化与剪贴板操作。调用方直接感知到了过多底层的 DOM 节点拼装与事件时序细节。

2. **破坏性全量重新渲染 (Destructive Streaming DOM Invalidation)**:
   流式传输每接收一个字符（Token），旧代码直接执行 `targetOutputEl.innerHTML = renderSafeMarkdown(currentTurnText)`。
   - **后果**: 每一帧都会销毁并重建内部所有 DOM 节点，导致并存的工具调用胶囊（Tool Call Capsule）被连带销毁，迫使主程序编写脆弱的 `insertBefore` 补丁在 DOM 树中重新寻找并挂载节点，彻底打断了用户展开工具参数或与生成式 UI 交互的操作焦点。

3. **工具调用链路表里不一 (Tool Call Boundary Disconnection)**:
   `tools.rs` 虽然定义了标准的 OpenAI Function Tools 描述，但实际网络请求 `stream_explanation` 未将 `tools` schema 注入 API Payload，也未处理 SSE 的 `delta.tool_calls` 事件，仅在离线 Fallback 分支做了硬编码字符串匹配，模块深度严重不足（对外暴露了工具能力，对内却未完成真实闭环）。

4. **伴生哲学契约冲突 (Ambient HUD vs. Modal Drawer)**:
   `GLOSSARY.md` 明确规定伴生 HUD 应规避弹出式对话框（Modal window）与侧边抽屉（Sidebar drawer）。此前采用的全屏遮罩式抽屉破坏了卡片的轻量通透感与菲涅尔折光美学。

---

## 2. 核心架构决策 (Decisions)

根据 John Ousterhout《软件设计哲学》（*A Philosophy of Software Design*）的**“深模块（Deep Module: 窄小纯粹的接口，深厚强悍的内聚封装）”**原则，重构核心子系统边界：

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Ambient HUD View Layer                          │
└──────────────────┬─────────────────────────────────┬───────────────────┘
                   │ (窄接口: 统一事件分发与显隐)       │ (窄接口: 纯状态装配)
┌──────────────────▼─────────────────┐   ┌────────────▼──────────────────┐
│  StreamingRenderPipeline (深模块)  │   │  SessionRepository (深模块)   │
│  - 块级分段增量构建 (Segment AST)    │   │  - 本地 LRU 存储与序列化管理  │
│  - 零破坏原位挂载 Tool/Widget 节点 │   │  - 模糊检索与时序倒排索引     │
│  - 严格免疫 XSS (零裸 innerHTML)   │   │  - 伴生透视笔记 Markdown 合成 │
└──────────────────┬─────────────────┘   └───────────────────────────────┘
                   │ (窄接口: Channel<StreamChunk>)
┌──────────────────▼─────────────────────────────────────────────────────┐
│                 CognitiveStreamEngine (Rust 后端深模块)                │
│  - 统一注入 tools schema 至实时 API Payload                            │
│  - 双向 SSE 状态机 (平滑聚合 delta.tool_calls 与流式 text)             │
│  - 沙箱执行器自动触发、反压调度与上下文反馈                             │
└────────────────────────────────────────────────────────────────────────┘
```

### 2.1 深模块 1：`CognitiveStreamEngine` (Rust 内核认知引擎)
- **窄接口 (Narrow Interface)**:
  ```rust
  pub async fn stream_explanation(req: QueryRequest, channel: Channel<StreamChunk>)
  ```
  调用方只需发起请求并接收流式 Chunk，完全不需要关心背后是在线大模型还是本地离线沙箱、是否调用了工具、工具调用发生过多少轮递归。
- **深内聚 (Deep Implementation)**:
  - 内部统一将 `tools.rs` 提供的工具规范声明融入 OpenAI / DeepSeek API 交互；
  - 自动识别并累积 SSE 响应中的 `tool_calls` 碎片，触发本地安全沙箱执行（文档查阅、位运算沙箱、宿主系统状态）；
  - 执行完毕后将结果封装为统一契约的 `StreamChunk::ToolCall` 发送前端，并视需要自动反哺上下文继续吐出洞察结论。

### 2.2 深模块 2：`StreamingRenderPipeline` (前端流式分段增量渲染器)
- **窄接口 (Narrow Interface)**:
  ```typescript
  export class StreamingRenderPipeline {
    constructor(container: HTMLElement);
    beginTurn(isFollowup: boolean, userQuery?: string): void;
    appendToken(token: string): void;
    mountToolCall(payload: ToolCallPayload): void;
    finish(): void;
  }
  ```
- **深内聚 (Deep Implementation)**:
  - **分段节点管理 (Segmented Block Nodes)**: 彻底废除流式过程中的全量 `innerHTML = ...` 覆写！
  - 内部将内容划分为独立的 DOM 挂载槽位：`TextSegmentEl`、`ToolCapsuleEl` 与 `WidgetSegmentEl`；
  - 接收普通文字 Token 时，仅更新当前 `TextSegmentEl`，绝对不破坏相邻已挂载的 Tool Capsule 节点或已展开的 Widget 卡片，用户在流式生成过程中点击交互 100% 保持焦点；
  - 彻底杜绝裸 `innerHTML`，所有操作均通过安全 DOM 节点或严格先行转义的流水线装配。

### 2.3 深模块 3：`SessionRepository` (会话历史持久化仓库)
- **窄接口 (Narrow Interface)**:
  ```typescript
  export class SessionRepository {
    static record(session: GlintSessionRecord): void;
    static query(filter?: string): GlintSessionRecord[];
    static get(id: string): GlintSessionRecord | undefined;
    static remove(id: string): void;
    static exportNotes(): string;
  }
  ```
- **深内聚 (Deep Implementation)**:
  - 封装本地持久化容量上限（LRU 100条自动淘汰）、防脏数据解析兜底、倒排时间索引；
  - 封装 Markdown 格式合成引擎，对外仅暴露一键取数与复制接口。

### 2.4 深模块 4：卡片内流光翻转透镜 (`In-Card Perspective Deck`) 替代侧栏抽屉
- 严格遵循 `GLOSSARY.md` 与 `ADR 0003`：移除全屏遮罩抽屉 DOM，采用卡片内部两面流光的**“透镜翻转 (Perspective Flip)”**或分段切换控制；
- 保持 HUD 伴生视窗的极简透明、零外部遮挡与单手瞬时操作特性。

---

## 3. 方案对比与评估 (Options Considered)

### 方案 A: 维持现有聚合在 main.ts 的单体模式，打补丁修复
| 维度 | 评估结果 |
| :--- | :--- |
| **复杂度** | 表面低，后期指数级递增 |
| **信息泄漏度** | 极高，所有时序状态在外部互相依赖 |
| **维护成本** | 高，修改流式逻辑极易破坏 Tool 挂载和历史存储 |

### 方案 B: 建立深模块（Deep Modules）子系统治理体系 (本决策)
| 维度 | 评估结果 |
| :--- | :--- |
| **复杂度** | 前期划分接口，后期极其清晰稳固 |
| **信息泄漏度** | 极低，每个子系统对外部仅暴露 3~4 个核心方法 |
| **运行稳定性** | 极高，增量 DOM 渲染彻底终结流式刷新冲突与节点丢失 |

---

## 4. 后续影响与收益 (Consequences)

### 正向收益 (What becomes easier)
1. **交互无闪烁、状态不丢失**: 流式输出时，Tool Call 展开框与 Generative UI 控件具备独立的生命周期槽位，用户即使在生成中点击展开工具入参，也绝不会被下一个 Token 冲掉。
2. **安全防线坚如磐石**: 所有的 DOM 拼装严格收口在 `StreamingRenderPipeline` 内部，主控制层杜绝任何 `innerHTML` 裸写调用。
3. **真实端到端 Tool Call 闭环**: Rust 端真正接入 live endpoint 工具协议支持，在线与离线行为统一。
4. **规范与 ADR 100% 对齐**: 彻底消除 `GLOSSARY.md` 违背项，伴生卡片体验重归纯粹。

### 需执行的落地任务 (Action Items)
1. [ ] 在 Rust 端 `src-tauri/src/llm.rs` 中将 `get_available_tools_schema()` 接入实时 API Payload，并实现 SSE `delta.tool_calls` 事件解析。
2. [ ] 在前端构建 `src/streaming-pipeline.ts`，重构为非破坏性分段增量 DOM 渲染引擎。
3. [ ] 重构 `src/main.ts`，将所有裸 `innerHTML`（包括复制按钮）改为安全 DOM/textContent 操作。
4. [ ] 升级 HUD 历史漫游界面，从侧栏抽屉模式重构为卡片内嵌式流光透视 Deck。
