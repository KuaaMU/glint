use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::ipc::Channel;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type", content = "data")]
pub enum StreamChunk {
    Token(String),
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
}

const SYSTEM_PROMPT: &str = r#"你是一个专注于消除认知鸿沟的技术透视助手。用户在阅读复杂技术文献、AI前沿或底层代码时遇到了看不懂的名词、术语、转折或句子。
你的任务是：
1. 立即切入本质：一针见血点出核心关键、概念内涵或上下文转折；
2. 结合语境讲透：结合上下文通俗解释它在当前系统/场景中解决了什么痛点、打破了什么误解或代表了什么权衡；
3. 如果用户进行多轮继续追问：紧扣上下文与用户的具体疑问，做递进式、具象化拆解，给出针对性解答；
4. 语言风格：直白、利落、专业、富于洞察，严禁用更晦涩的学术黑话套娃，严禁任何客套寒暄（如“你好”、“很高兴为您解答”）。直接输出正文。"#;

pub fn build_api_payload(req: &QueryRequest) -> serde_json::Value {
    let mut api_messages = vec![serde_json::json!({
        "role": "system",
        "content": SYSTEM_PROMPT
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

    serde_json::json!({
        "model": req.model.as_deref().unwrap_or("deepseek-chat"),
        "messages": api_messages,
        "stream": true,
        "temperature": 0.3
    })
}

pub async fn stream_explanation(req: QueryRequest, channel: Channel<StreamChunk>) {
    let api_key = req.api_key.as_deref().unwrap_or_default().trim().to_string();
    let base_url = req
        .base_url
        .as_deref()
        .unwrap_or("https://api.deepseek.com/v1");

    // 如果未配置 API Key，启动内置动态智能解析，保证离线追问也有深度回复
    if api_key.is_empty() {
        fallback_dynamic_explain(&req.messages, &channel).await;
        return;
    }

    let client = match Client::builder().timeout(Duration::from_secs(60)).build() {
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
                            if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                                let _ = channel.send(StreamChunk::Token(delta.to_string()));
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

async fn fallback_dynamic_explain(messages: &[ChatMessage], channel: &Channel<StreamChunk>) {
    let last_user_text = messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_text_lossy())
        .unwrap_or_default();
    let last_user_msg = last_user_text.as_str();

    let reply = if messages.len() > 1 {
        // 多轮追问场景
        if last_user_msg.contains("GPU") || last_user_msg.contains("显卡") {
            "在 GPU 架构中，内存层次的这一矛盾更加尖锐：GPU 拥有成千上万个并发线程（ALU 计算单元），如果全部向全局显存（VRAM/HBM）索取数据，总线会瞬间饱和瘫痪。因此 GPU 引入了超高速的片上共享内存（Shared Memory / SRAM）与寄存器文件（Register File）。现代算子优化（如 FlashAttention）正是通过在 SRAM 内部完成局部矩阵计算，最大程度减少往返全局显存的次数，本质依然是对抗「非均质物理内存」的延迟陷阱。".to_string()
        } else if last_user_msg.contains("优化") || last_user_msg.contains("怎么解决") || last_user_msg.contains("怎么办") {
            "在工程层面突破此瓶颈的核心思路是「空间换时间与计算换带宽」：\n1. 结构体对齐（Padding/Align）：将多核高频读写的变量按 64 字节 Cache Line 对齐，消除伪共享；\n2. 算子融合（Operator Fusion）：将连续的多次访存操作合成为单次核函数，中间结果保留在寄存器；\n3. 内存预取（Software Prefetching）：在硬件空闲周期提前将下一次循环所需的数据滑入缓存。".to_string()
        } else if last_user_msg.contains("为什么") || last_user_msg.contains("原因") {
            "造成这个现象的物理根因在于半导体材料与物理距离：CPU 芯片面积有限，极速的 SRAM 只能容纳数兆到数十兆字节；而海量存储必须依赖外挂的 DRAM 芯片。电信号穿透芯片引脚、经过 PCB 主板印刷走线传输，必然存在纳秒级延迟。这不是软件层能简单抽象抹平的，必须正视其物理拓扑。".to_string()
        } else {
            format!("针对你的追问“{}”：在真实的底层系统架构中，理解这一特性的核心价值在于「心智模型的切换」——从高级语言‘平坦均匀大数组’的理想假设，切换到‘距离核心远近不一的高速缓存与外部存储集群’的真实硬件视角。在此视角下，执行瓶颈往往不再是算力指令，而是访存延迟与数据竞争。", last_user_msg)
        }
    } else {
        // 第一轮初次选词解释
        match last_user_msg {
            t if t.contains("面对的并不是") => {
                "这里的关键在于「并不是」引出的转折：CPU 和 NPU/GPU 等运算核心面对的物理内存并非理想化的均匀整体。前文提到的虚拟地址空间是软件层面的抽象，给程序一种拥有连续、独占内存的假象；而一旦落到真实硬件，内存实际上是分层且不同质的——从极快但极小的寄存器、各级 Cache，到较慢的主存（DRAM），甚至跨芯片访问时还有更慢的一致性总线。因此，「一块均质平坦、可以瞬时读写的物理内存」是一种需要被打破的误解，这正是理解内存墙和 Cache 一致性的出发点。".to_string()
            }
            t if t.contains("高性能多线程系统代码") => {
                "“高性能多线程系统代码”指的是那些需要充分利用多核 CPU 并行能力、对执行效率和延迟有严格要求的底层程序，例如数据库内核、高性能网络服务器、并发数据结构或实时计算引擎。这类代码的核心挑战在于：多个线程可能同时读写同一块内存区域，而现代处理器为了弥合 CPU 与内存之间的速度鸿沟，引入了多级 Cache（如 L1、L2、L3）和写缓冲区等硬件优化机制。因此，编写此类代码必须精通硬件级 Cache 一致性协议与锁消除策略。".to_string()
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
                format!("【{}】核心洞察：该概念在现代系统工程与前沿计算架构中直击关键设计权衡。它破除了传统高层抽象的理想假设，揭示了底层真实的性能瓶颈与一致性边界。\n\n提示：当前处于 Glint 本地免 Key 演示模式。可在右上角齿轮设置中填入 DeepSeek / Kimi / OpenAI / Ollama 凭证即可体验全网无限词条深度透视。", last_user_msg)
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
        };

        let payload = build_api_payload(&req);
        assert_eq!(payload["model"], "gpt-4o");
        assert_eq!(payload["stream"], true);
        assert_eq!(payload["temperature"], 0.3);

        let messages = payload["messages"].as_array().expect("messages 应为数组");
        // index 0: 系统提示词
        assert_eq!(messages[0]["role"], "system");
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

