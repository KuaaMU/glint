use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::ipc::Channel;

use crate::tools::{execute_tool, get_available_tools_schema, ToolCallPayload};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", content = "data")]
pub enum StreamChunk {
    Token(String),
    ToolCall(ToolCallPayload),
    Done,
    Error(String),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    pub fn as_text_lossy(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Parts(parts) => {
                let texts: Vec<&str> = parts
                    .iter()
                    .filter_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                texts.join(" ")
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrlPayload },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ImageUrlPayload {
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: MessageContent,
}

#[allow(dead_code)]
impl ChatMessage {
    pub fn user_text(text: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: MessageContent::Text(text.into()),
        }
    }

    pub fn user_multimodal(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: MessageContent::Parts(vec![
                ContentPart::Text {
                    text: text.into(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrlPayload {
                        url: image_url.into(),
                    },
                },
            ]),
        }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: MessageContent::Text(text.into()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct QueryRequest {
    pub messages: Vec<ChatMessage>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub window_title: Option<String>,
}

const SYSTEM_PROMPT: &str = r#"你是 Glint（微光）—— 桌面原生的高级认知透视伴生体。
你的核心职责：结合用户所处的真实桌面应用上下文（网页、代码、文档、设计工具等），消除认知鸿沟，一针见血讲透事物本质。

输出铁律：
1. 严禁好为人师的说教腔与模板废话（严禁“在你读的语境里”、“我们来看一下”、“拆开看”、“这句话的意思是”等任何 AI 套话）；
2. 严禁客套寒暄（如“你好”、“很高兴为你解答”），第一句直接切入核心结论与场景事实；
3. 上下文深度锚定：若给出了宿主应用上下文（如当前是 Bilibili 网站、arXiv 论文、VS Code 代码等），必须紧扣该具体场景下的真实产品逻辑、系统机制或技术权衡；
4. 语言质感：如同资深架构师或首席设计师在耳边的敏锐提点，精炼、锋利、富于洞察，直给正文。

【生成式 UI 与可视化呈现规范】
遇有方案对比、时序流程、技术复杂度或架构警示时，可按需输出以下结构化组件代码块，桌面端将自动渲染为晶透琉璃可视化卡片：
- 方案对比：```widget:compare\n{"title":"方案对比","columns":["维度","方案A","方案B"],"rows":[["性能","...","..."]]}\n```
- 时序流程：```widget:flow\n{"title":"核心流程","steps":[{"title":"环节1","desc":"..."},{"title":"环节2","desc":"..."}]}\n```
- 复杂度与特性指标：```widget:metrics\n{"title":"技术特征","items":[{"label":"时间复杂度","value":"O(log N)","tone":"cyan"}]}\n```
- 核心警示与避坑指南：```widget:callout\n{"type":"warning","title":"风险提示","content":"..."}\n```"#;

pub fn build_api_payload(req: &QueryRequest) -> serde_json::Value {
    let sys_prompt = if let Some(title) = &req.window_title {
        if !title.trim().is_empty() {
            format!(
                "{}\n\n【用户当前宿主环境】\n前台应用窗口: {}\n请务必结合该宿主软件场景（如特定网站、代码编辑器、技术文献等）提供具备真实情境感知力的精准释义，绝不脱离实际上下文讲空话。",
                SYSTEM_PROMPT,
                title.trim()
            )
        } else {
            SYSTEM_PROMPT.to_string()
        }
    } else {
        SYSTEM_PROMPT.to_string()
    };

    let mut api_messages = vec![serde_json::json!({
        "role": "system",
        "content": sys_prompt
    })];

    for msg in &req.messages {
        let content_val = match &msg.content {
            MessageContent::Text(t) => serde_json::Value::String(t.clone()),
            MessageContent::Parts(parts) => {
                serde_json::to_value(parts).unwrap_or(serde_json::Value::Array(vec![]))
            }
        };

        api_messages.push(serde_json::json!({
            "role": msg.role,
            "content": content_val
        }));
    }

    let mut payload = serde_json::json!({
        "model": req.model.as_deref().unwrap_or("deepseek-chat"),
        "messages": api_messages,
        "stream": true,
        "temperature": 0.3
    });

    let tools = get_available_tools_schema();
    if !tools.is_empty() {
        payload["tools"] = serde_json::Value::Array(tools);
    }

    payload
}

pub async fn stream_explanation(req: QueryRequest, channel: Channel<StreamChunk>) {
    let api_key = req.api_key.as_deref().unwrap_or_default().trim().to_string();
    let base_url = req
        .base_url
        .as_deref()
        .unwrap_or("https://api.deepseek.com/v1");

    // 如果未配置 API Key，启动内置动态智能解析，保证离线追问也有深度回复
    if api_key.is_empty() {
        fallback_dynamic_explain(&req.messages, req.window_title.as_deref(), &channel).await;
        return;
    }

    // 建立健壮的流式 HTTP/1.1 客户端，规避代理环境下 HTTP/2 多路复用流异常重置 (error decoding response body)
    let client = match Client::builder()
        .http1_only()
        .tcp_keepalive(Duration::from_secs(15))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = channel.send(StreamChunk::Error(e.to_string()));
            return;
        }
    };

    let endpoint = if base_url.ends_with("/chat/completions") {
        base_url.to_string()
    } else {
        format!("{}/chat/completions", base_url.trim_end_matches('/'))
    };

    let body = build_api_payload(&req);

    let res = match client
        .post(&endpoint)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = channel.send(StreamChunk::Error(format!("请求服务失败: {}", e)));
            return;
        }
    };

    let status = res.status();
    if !status.is_success() {
        let err_txt = res.text().await.unwrap_or_default();
        let _ = channel.send(StreamChunk::Error(format!("API 报错 (HTTP {}): {}", status, err_txt)));
        return;
    }

    let mut stream = res.bytes_stream();
    let mut buffer = String::new();

    while let Some(item) = stream.next().await {
        match item {
            Ok(bytes) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.starts_with("data: ") {
                        let data = &line["data: ".len()..];
                        if data == "[DONE]" {
                            let _ = channel.send(StreamChunk::Done);
                            return;
                        }
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(choice) = v["choices"].as_array().and_then(|c| c.first()) {
                                let delta = &choice["delta"];

                                // 1. 解析实时 Tool Call
                                if let Some(tool_calls) = delta["tool_calls"].as_array() {
                                    for tc in tool_calls {
                                        let func = &tc["function"];
                                        if let Some(name) = func["name"].as_str() {
                                            let args = func["arguments"].as_str().unwrap_or("{}");
                                            let _ = channel.send(StreamChunk::ToolCall(ToolCallPayload {
                                                name: name.to_string(),
                                                args: args.to_string(),
                                                status: "calling".to_string(),
                                                result: None,
                                            }));

                                            let res = execute_tool(name, args, req.window_title.as_deref());
                                            let _ = channel.send(StreamChunk::ToolCall(ToolCallPayload {
                                                name: name.to_string(),
                                                args: args.to_string(),
                                                status: "done".to_string(),
                                                result: Some(res),
                                            }));
                                        }
                                    }
                                }

                                // 2. 解析文本 Token
                                if let Some(content) = delta["content"].as_str() {
                                    let _ = channel.send(StreamChunk::Token(content.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = channel.send(StreamChunk::Error(format!("流式传输异常中断: {}", e)));
                return;
            }
        }
    }

    let _ = channel.send(StreamChunk::Done);
}

async fn fallback_dynamic_explain(
    messages: &[ChatMessage],
    window_title: Option<&str>,
    channel: &Channel<StreamChunk>,
) {
    let last_user_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_text_lossy())
        .unwrap_or_default();
    let last_user_msg = last_user_text.as_str();

    // 检测是否适合触发工具调用示例 (Tool Call)
    let tool_trigger: Option<(&str, String)> = if last_user_msg.contains("0x") || last_user_msg.contains(">>") || last_user_msg.contains("<<") {
        Some(("calc_sandbox", format!(r#"{{"expression": "{}"}}"#, last_user_msg.trim())))
    } else if last_user_msg.contains("B-Tree") || last_user_msg.contains("LSM") || last_user_msg.contains("树") {
        Some(("lookup_doc", r#"{"query": "B-Tree vs LSM-Tree", "domain": "database"}"#.to_string()))
    } else if last_user_msg.contains("零拷贝") || last_user_msg.contains("Zero-Copy") || last_user_msg.contains("mmap") {
        Some(("lookup_doc", r#"{"query": "Zero-Copy sendfile", "domain": "linux"}"#.to_string()))
    } else if last_user_msg.contains("tokio") || last_user_msg.contains("mpsc") {
        Some(("lookup_doc", r#"{"query": "tokio::sync::mpsc", "domain": "rust"}"#.to_string()))
    } else if messages.len() == 1 && !last_user_msg.is_empty() {
        Some(("lookup_doc", format!(r#"{{"query": "{}"}}"#, last_user_msg.trim())))
    } else {
        None
    };

    if let Some((tool_name, tool_args)) = tool_trigger {
        // 1. 发送工具调用开始信号
        let _ = channel.send(StreamChunk::ToolCall(ToolCallPayload {
            name: tool_name.to_string(),
            args: tool_args.clone(),
            status: "calling".to_string(),
            result: None,
        }));
        tokio::time::sleep(Duration::from_millis(120)).await;

        // 2. 执行工具调用并发送完成信号
        let tool_res = execute_tool(tool_name, &tool_args, window_title);
        let _ = channel.send(StreamChunk::ToolCall(ToolCallPayload {
            name: tool_name.to_string(),
            args: tool_args,
            status: "done".to_string(),
            result: Some(tool_res),
        }));
        tokio::time::sleep(Duration::from_millis(80)).await;
    }

    let reply = if messages.len() > 1 {
        // 多轮追问场景
        if last_user_msg.contains("GPU") || last_user_msg.contains("显卡") {
            "在 GPU 架构中，内存层次的这一矛盾更加尖锐：GPU 拥有成千上万个并发线程（ALU 计算单元），如果全部向全局显存（VRAM/HBM）索取数据，总线会瞬间饱和瘫痪。因此 GPU 引入了超高速的片上共享内存（Shared Memory / SRAM）与寄存器文件（Register File）。\n\n```widget:flow\n{\n  \"title\": \"GPU 算子局部访存流水线\",\n  \"steps\": [\n    {\"title\": \"全局显存 VRAM\", \"desc\": \"百GB容量，延迟高 (200~400ns)\"},\n    {\"title\": \"片上 SRAM / Shared Mem\", \"desc\": \"微秒内分块载入 (Tiling)，极低延迟 (10~20ns)\"},\n    {\"title\": \"Tensor Core 矩阵乘\", \"desc\": \"在寄存器与SRAM内完成局部核函数计算\"}\n  ]\n}\n```\n\n现代算子优化（如 FlashAttention）正是通过在 SRAM 内部完成局部矩阵计算，最大程度减少往返全局显存的次数，本质依然是对抗「非均质物理内存」的延迟陷阱。".to_string()
        } else if last_user_msg.contains("优化") || last_user_msg.contains("怎么解决") || last_user_msg.contains("怎么办") {
            "在工程层面突破此瓶颈的核心思路是「空间换时间与计算换带宽」：\n\n```widget:callout\n{\n  \"type\": \"tip\",\n  \"title\": \"架构师调优基线\",\n  \"content\": \"优先做 Cache Line 64B 对齐消除伪共享，其次做算子融合减少显存往返，最后评估软件预取。\"\n}\n```\n\n1. 结构体对齐（Padding/Align）：将多核高频读写的变量按 64 字节 Cache Line 对齐，消除伪共享；\n2. 算子融合（Operator Fusion）：将连续的多次访存操作合成为单次核函数，中间结果保留在寄存器；\n3. 内存预取（Software Prefetching）：在硬件空闲周期提前将下一次循环所需的数据滑入缓存。".to_string()
        } else if last_user_msg.contains("为什么") || last_user_msg.contains("原因") {
            "造成这个现象的物理根因在于半导体材料与物理距离：CPU 芯片面积有限，极速的 SRAM 只能容纳数兆到数十兆字节；而海量存储必须依赖外挂的 DRAM 芯片。电信号穿透芯片引脚、经过 PCB 主板印刷走线传输，必然存在纳秒级延迟。这不是软件层能简单抽象抹平的，必须正视其物理拓扑。".to_string()
        } else if let Some(w) = window_title {
            format!("针对在「{}」情境下的追问“{}”：从高层业务逻辑下沉到底层物理模型，核心在于打破平坦抽象，根据实际场景特征在延迟与吞吐之间做出精确权衡。", w, last_user_msg)
        } else {
            format!("针对追问“{}”：核心在于心智模型的切换——从高级语言‘平坦均匀大数组’的理想假设，切换到‘距离核心远近不一的高速缓存与外部存储集群’的真实物理拓扑，瓶颈往往在于访存延迟而非算力指令。", last_user_msg)
        }
    } else {
        // 第一轮初次选词解释
        match last_user_msg {
            t if t.contains("下载客户端") => {
                if let Some(w) = window_title {
                    if w.contains("bilibili") || w.contains("哔哩哔哩") {
                        "在哔哩哔哩当前网页场景中，这是引导安装专属桌面客户端的入口。桌面客户端支持 4K 超清硬件加速解码、更低系统功耗、无浏览器标签页干扰常驻，并提供视频离线缓存下载。\n\n```widget:compare\n{\n  \"title\": \"Bilibili Web 端 vs 桌面客户端\",\n  \"columns\": [\"能力维度\", \"浏览器 Web 端\", \"原生桌面端\"],\n  \"rows\": [\n    [\"硬件解码\", \"受限于浏览器沙箱\", \"直接调用独立 GPU 4K/8K 解码\"],\n    [\"功耗与常驻\", \"高内存开销与标签干扰\", \"极轻量常驻、支持快捷键与离线缓存\"]\n  ]\n}\n```".to_string()
                    } else {
                        format!("在「{}」应用场景中，该入口用于安装专属桌面客户端。相较网页版，独立客户端拥有原生硬件加速解码、本地磁盘直接缓存及更低系统资源占用。", w)
                    }
                } else {
                    "该入口用于安装专属桌面客户端。相比网页端，独立客户端具备原生 GPU 硬件加速解码、本地磁盘离线缓存及更低的系统资源开销。".to_string()
                }
            }
            t if t.contains("B-Tree") || t.contains("LSM") => {
                "B-Tree 与 LSM-Tree 是现代存储引擎的两大基本派系，分别代表了对「随机点读」与「顺序写吞吐」的极致权衡：\n\n```widget:compare\n{\n  \"title\": \"B-Tree vs LSM-Tree 存储引擎选型矩阵\",\n  \"columns\": [\"对比维度\", \"B-Tree (InnoDB / SQLite)\", \"LSM-Tree (RocksDB / ClickHouse)\"],\n  \"rows\": [\n    [\"写入模型\", \"原地更新 (In-place Update)\", \"追加写 (Append-Only WAL + MemTable)\"],\n    [\"读性能\", \"极高点读，O(log N) 稳定\", \"较慢点读，需经过多层 SSTable 与布隆过滤器\"],\n    [\"写放大与吞吐\", \"随机写易引起页分裂，写放大约 10~30x\", \"顺序写入吞吐极高，后台异步 Compaction\"]\n  ]\n}\n```\n\n- **B-Tree 核心优势**：适合读多写少、事务强一致性场景；\n- **LSM-Tree 核心优势**：适合高吞吐时序数据、日志追加与海量写操作。".to_string()
            }
            t if t.contains("面对的并不是") => {
                "这里的关键在于「并不是」引出的转折：CPU 和 NPU/GPU 等运算核心面对的物理内存并非理想化的均匀整体。前文提到的虚拟地址空间是软件层面的抽象，给程序一种拥有连续、独占内存的假象；而一旦落到真实硬件，内存实际上是分层且不同质的——从极快但极小的寄存器、各级 Cache，到较慢的主存（DRAM），甚至跨芯片访问时还有更慢的一致性总线。\n\n```widget:metrics\n{\n  \"title\": \"现代芯片多级存储金字塔延迟阶梯\",\n  \"items\": [\n    {\"label\": \"L1 Cache\", \"value\": \"~1 ns (0.5m)\", \"tone\": \"cyan\"},\n    {\"label\": \"L2/L3 Cache\", \"value\": \"~4-10 ns (5m)\", \"tone\": \"cyan\"},\n    {\"label\": \"主存 DRAM\", \"value\": \"~60-100 ns (100m)\", \"tone\": \"amber\"},\n    {\"label\": \"NVMe SSD\", \"value\": \"~10,000 ns (10km)\", \"tone\": \"rose\"}\n  ]\n}\n```\n\n因此，「一块均质平坦、可以瞬时读写的物理内存」是一种需要被打破的误解，这正是理解内存墙和 Cache 一致性的出发点。".to_string()
            }
            t if t.contains("高性能多线程系统代码") => {
                "“高性能多线程系统代码”指的是那些需要充分利用多核 CPU 并行能力、对执行效率和延迟有严格要求的底层程序，例如数据库内核、高性能网络服务器、并发数据结构或实时计算引擎。编写此类代码必须精通硬件级 Cache 一致性协议与锁消除策略。".to_string()
            }
            t if t.contains("内存墙") => {
                "“内存墙”（Memory Wall）指的是处理器计算能力（算力）的增速远远甩开了内存访问速度（带宽与延迟）的增速所形成的物理瓶颈。运算核心一条 ALU 指令可能仅需零点几纳秒，而一次远端 DRAM 内存读取却需耗时数十甚至上百纳秒。这导致大量计算核心被迫处于长时间的等待停顿状态（Memory Stalling），也是当前大模型算子优化极力推崇算子融合与片上分块的根本原因。".to_string()
            }
            t if t.contains("Tiling") || t.contains("分块") => {
                "“Tiling 分块优化”是一种将大规模数据矩阵划分为适合各级高速缓存（L1/L2 Cache 或 GPU 共享内存）尺寸的局部小块进行分阶段计算的经典技术。它的核心目标是最大化数据的“时间局部性”与“空间局部性”，避免数据刚被加载进缓存又被过快逐出，从而使内存带宽瓶颈转变为纯计算吞吐。".to_string()
            }
            t if t.contains("Cache 一致性") || t.contains("Cache Coherence") => {
                "“Cache 一致性”（Cache Coherence）指的是在多核处理器系统中，确保所有核心的私有 Cache 对同一块主存数据看到统一真实状态的硬件协议机制。当核心 A 修改了自己 L1 Cache 中的某变量，核心 B 的 Cache 必须通过总线监听（如 MESI 协议）立刻察觉并将旧副本标为无效，防止多线程程序读到脏数据。".to_string()
            }
            _ => {
                if let Some(w) = window_title {
                    format!("【{}】在「{}」情境下：直击该场景的核心机制与操作目的。\n\n提示：可在右上角设置中填入 API Key 体验全球前沿模型的无限泛化透视能力。", last_user_msg, w)
                } else {
                    format!("【{}】直击核心本质与关键设计权衡。\n\n提示：可在右上角设置中填入 API Key 体验全网无限词条深度透视。", last_user_msg)
                }
            }
        }
    };

    for ch in reply.chars() {
        let _ = channel.send(StreamChunk::Token(ch.to_string()));
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let _ = channel.send(StreamChunk::Done);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text_message_deserialization() {
        let json_str = r#"{"role": "user", "content": "测试纯文本选词"}"#;
        let msg: ChatMessage = serde_json::from_str(json_str).expect("应当正确反序列化纯文本消息");
        assert_eq!(msg.role, "user");
        assert_eq!(msg.content.as_text_lossy(), "测试纯文本选词");
    }

    #[test]
    fn test_assistant_message() {
        let msg = ChatMessage::assistant("回复测试");
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content.as_text_lossy(), "回复测试");
    }

    #[test]
    fn test_multimodal_message_deserialization() {
        let json_str = r#"{
            "role": "user",
            "content": [
                {"type": "text", "text": "请分析架构图重点"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,AAAA"}}
            ]
        }"#;
        let msg: ChatMessage = serde_json::from_str(json_str).expect("应当正确反序列化多模态消息");
        assert_eq!(msg.role, "user");
        match &msg.content {
            MessageContent::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                match &parts[0] {
                    ContentPart::Text { text } => assert_eq!(text, "请分析架构图重点"),
                    _ => panic!("第 0 项应为 Text"),
                }
                match &parts[1] {
                    ContentPart::ImageUrl { image_url } => assert_eq!(image_url.url, "data:image/png;base64,AAAA"),
                    _ => panic!("第 1 项应为 ImageUrl"),
                }
            }
            _ => panic!("应为 MessageContent::Parts"),
        }
    }

    #[test]
    fn test_build_api_payload_with_mixed_messages() {
        let req = QueryRequest {
            messages: vec![
                ChatMessage::user_text("第一轮文字提问"),
                ChatMessage::user_multimodal("第二轮看图提问", "data:image/png;base64,BBBB"),
            ],
            api_key: Some("sk-test".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            model: Some("gpt-4o".to_string()),
            window_title: Some("Bilibili - Google Chrome".to_string()),
        };

        let payload = build_api_payload(&req);
        assert_eq!(payload["model"], "gpt-4o");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["temperature"], 0.3);

        let messages = payload["messages"].as_array().expect("messages 应为数组");
        // index 0: 系统提示词包含宿主窗口上下文
        assert!(messages[0]["content"].as_str().unwrap().contains("Bilibili - Google Chrome"));
        assert!(messages[0]["content"].as_str().unwrap().contains("认知鸿沟"));

        // index 1: 纯文本
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"], "第一轮文字提问");

        // index 2: 多模态
        assert_eq!(messages[2]["role"], "user");
        let parts = messages[2]["content"].as_array().expect("多模态 content 应为数组");
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[0]["text"], "第二轮看图提问");
        assert_eq!(parts[1]["type"], "image_url");
        assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,BBBB");
    }
}

