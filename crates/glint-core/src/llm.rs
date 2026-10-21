use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use crate::tools::{execute_tool, get_available_tools_schema, ToolCallPayload};

/// Narrow sink interface: the core never knows whether the consumer is a
/// Tauri IPC channel, a Slint property, or a CLI stdout writer.
pub trait StreamSink: Send + Sync + 'static {
    fn send(&self, chunk: StreamChunk) -> Result<(), String>;
}

impl StreamSink for tokio::sync::mpsc::UnboundedSender<StreamChunk> {
    fn send(&self, chunk: StreamChunk) -> Result<(), String> {
        tokio::sync::mpsc::UnboundedSender::send(self, chunk).map_err(|e| e.to_string())
    }
}

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

pub async fn stream_explanation(req: QueryRequest, sink: Arc<dyn StreamSink>) {
    let api_key = req.api_key.as_deref().unwrap_or_default().trim().to_string();
    let base_url = req
        .base_url
        .as_deref()
        .unwrap_or("https://api.deepseek.com/v1");

    // Never impersonate a model. Without a key the honest state is "offline /
    // not configured"; the UI renders this as an explicit notice.
    if api_key.is_empty() {
        let _ = sink.send(StreamChunk::Error(
            "未配置 API Key。请在偏好设置中填写，或切换到本地模型。".to_string(),
        ));
        let _ = sink.send(StreamChunk::Done);
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
            let _ = sink.send(StreamChunk::Error(e.to_string()));
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
            let _ = sink.send(StreamChunk::Error(format!("请求服务失败: {}", e)));
            return;
        }
    };

    let status = res.status();
    if !status.is_success() {
        let err_txt = res.text().await.unwrap_or_default();
        let _ = sink.send(StreamChunk::Error(format!("API 报错 (HTTP {}): {}", status, err_txt)));
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
                            let _ = sink.send(StreamChunk::Done);
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
                                            let _ = sink.send(StreamChunk::ToolCall(ToolCallPayload {
                                                name: name.to_string(),
                                                args: args.to_string(),
                                                status: "calling".to_string(),
                                                result: None,
                                            }));

                                            let res = execute_tool(name, args, req.window_title.as_deref());
                                            let _ = sink.send(StreamChunk::ToolCall(ToolCallPayload {
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
                                    let _ = sink.send(StreamChunk::Token(content.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = sink.send(StreamChunk::Error(format!("流式传输异常中断: {}", e)));
                return;
            }
        }
    }

    let _ = sink.send(StreamChunk::Done);
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

