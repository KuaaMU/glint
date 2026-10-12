use crate::config::{load_config, save_config, AppConfig};
use reqwest::Client;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

#[tauri::command]
pub fn open_preferences_window(app: AppHandle) -> Result<(), String> {
    if let Some(pref_win) = app.get_webview_window("preferences") {
        let _ = pref_win.unminimize();
        let _ = pref_win.show();
        let _ = pref_win.set_focus();
        Ok(())
    } else {
        let app_handle = app.clone();
        app.run_on_main_thread(move || {
            let win = WebviewWindowBuilder::new(
                &app_handle,
                "preferences",
                WebviewUrl::App("preferences.html".into()),
            )
            .title("Glint 偏好设置")
            .inner_size(620.0, 700.0)
            .min_inner_size(480.0, 480.0)
            .resizable(true)
            .center()
            .build();

            if let Ok(w) = win {
                let _ = w.show();
                let _ = w.set_focus();
            }
        })
        .map_err(|e| format!("调度主线程创建偏好设置失败: {:?}", e))?;
        Ok(())
    }
}

#[tauri::command]
pub fn hide_preferences_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("preferences") {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn get_app_config(app: AppHandle) -> AppConfig {
    load_config(&app)
}

#[tauri::command]
pub fn update_app_config(app: AppHandle, config: AppConfig) -> Result<(), String> {
    save_config(&app, &config)?;
    if let Some(hud) = app.get_webview_window("hud") {
        let _ = hud.set_always_on_top(config.always_on_top);
    }
    // 通知全部窗口（包括伴生 HUD 卡片）同步配置
    let _ = app.emit("glint-settings-changed", &config);
    Ok(())
}

#[tauri::command]
pub async fn fetch_models(api_key: String, base_url: String) -> Result<Vec<String>, String> {
    let client = Client::builder()
        .timeout(Duration::from_millis(3500))
        .build()
        .map_err(|e| format!("初始化客户端失败: {}", e))?;

    let trimmed_url = base_url.trim().trim_end_matches('/');
    if trimmed_url.is_empty() {
        return Err("Base URL 不能为空".to_string());
    }

    let key = api_key.trim();
    let mut candidate_urls = Vec::new();

    if trimmed_url.ends_with("/v1") {
        candidate_urls.push(format!("{}/models", trimmed_url));
        candidate_urls.push(format!("{}/models", trimmed_url.trim_end_matches("/v1")));
    } else {
        candidate_urls.push(format!("{}/v1/models", trimmed_url));
        candidate_urls.push(format!("{}/models", trimmed_url));
    }

    // 同时添加 Ollama 探测端点
    let ollama_tag_url = format!("{}/api/tags", trimmed_url.trim_end_matches("/v1"));
    candidate_urls.push(ollama_tag_url);

    let mut last_error_msg = String::new();
    let mut model_ids: Vec<String> = Vec::new();

    for endpoint in candidate_urls {
        let mut req = client.get(&endpoint);
        if !key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        if let Ok(resp) = req.send().await {
            let status = resp.status();
            if status.is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    // 1. 标准 OpenAI/DeepSeek data 字段
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        for item in data {
                            if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                                model_ids.push(id.to_string());
                            } else if let Some(s) = item.as_str() {
                                model_ids.push(s.to_string());
                            }
                        }
                    }
                    // 2. Ollama models 字段
                    if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                        for item in models {
                            if let Some(name) = item.get("name").and_then(|s| s.as_str()) {
                                model_ids.push(name.to_string());
                            }
                        }
                    }

                    if !model_ids.is_empty() {
                        break;
                    }
                }
            } else if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                last_error_msg = format!("认证失败 (HTTP {}): API Key 无效或未授权", status);
            } else {
                last_error_msg = format!("端点返回异常状态 (HTTP {})", status);
            }
        }
    }

    if model_ids.is_empty() {
        if !last_error_msg.is_empty() {
            return Err(last_error_msg);
        }
        return Err("未能从该端点探测到模型列表，请确认 Base URL 与 API Key 是否有效".to_string());
    }

    // 常用对话模型优先展示
    model_ids.sort_by(|a, b| {
        let is_chat_a = a.contains("chat") || a.contains("reasoner") || a.contains("gpt-4") || a.contains("qwen") || a.contains("claude");
        let is_chat_b = b.contains("chat") || b.contains("reasoner") || b.contains("gpt-4") || b.contains("qwen") || b.contains("claude");
        is_chat_b.cmp(&is_chat_a).then(a.cmp(b))
    });
    model_ids.dedup();

    Ok(model_ids)
}

#[tauri::command]
pub async fn test_llm_connection(
    api_key: String,
    base_url: String,
    model: String,
) -> Result<String, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let trimmed = base_url.trim().trim_end_matches('/');
    let endpoints = if trimmed.ends_with("/chat/completions") {
        vec![trimmed.to_string()]
    } else if trimmed.ends_with("/v1") {
        vec![
            format!("{}/chat/completions", trimmed),
            format!("{}/chat/completions", trimmed.trim_end_matches("/v1")),
        ]
    } else {
        vec![
            format!("{}/v1/chat/completions", trimmed),
            format!("{}/chat/completions", trimmed),
        ]
    };

    let body = serde_json::json!({
        "model": model,
        "messages": [
            { "role": "user", "content": "ping" }
        ],
        "max_tokens": 5
    });

    let start = Instant::now();
    let mut last_err = String::new();

    for endpoint in endpoints {
        let res = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", api_key.trim()))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        match res {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let elapsed = start.elapsed().as_millis();
                    return Ok(format!("连接成功 (HTTP 200, 延迟 {}ms)", elapsed));
                } else {
                    let err_text = resp.text().await.unwrap_or_default();
                    let short_err = if err_text.len() > 100 {
                        format!("{}...", &err_text[..100])
                    } else {
                        err_text
                    };
                    last_err = format!("服务返回 HTTP {}: {}", status, short_err);
                }
            }
            Err(e) => {
                last_err = format!("连接超时或失败: {}", e);
            }
        }
    }

    Err(last_err)
}
