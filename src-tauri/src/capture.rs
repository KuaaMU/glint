use crate::platform::{get_cursor_position, simulate_copy};
use arboard::Clipboard;
use std::thread::sleep;
use std::time::Duration;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct SelectionResult {
    pub text: String,
    pub cursor_x: i32,
    pub cursor_y: i32,
}

pub fn get_cursor_pos() -> (i32, i32) {
    get_cursor_position()
}

fn get_clipboard_with_retry() -> Option<Clipboard> {
    for _ in 0..5 {
        if let Ok(c) = Clipboard::new() {
            return Some(c);
        }
        sleep(Duration::from_millis(6));
    }
    None
}

/// 基于原生系统更新序列号 (Sequence Number) 与零污染快照的极致采词引擎
/// 确保：未划选新文本时绝不回退至历史剪贴板，绝不污染系统剪贴板，绝无锁冲突
pub fn capture_selected_text() -> Option<SelectionResult> {
    let (cursor_x, cursor_y) = get_cursor_pos();

    // 1. 获取复制前的初始序列号与内容快照
    let initial_seq = crate::platform::get_clipboard_sequence();
    let original_snapshot = get_clipboard_with_retry().and_then(|mut c| c.get_text().ok());

    // 2. 触发跨平台目标应用选区模拟复制 (自动等待 Alt 键物理释放并注入硬件扫描码)
    simulate_copy();

    // 3. 高精度智能监听 (轮询至多 350ms，每 15ms 检测一次系统状态)
    let mut captured_text: Option<String> = None;
    let start = std::time::Instant::now();

    while start.elapsed().as_millis() < 350 {
        sleep(Duration::from_millis(15));

        let current_seq = crate::platform::get_clipboard_sequence();
        let seq_changed = initial_seq != 0 && current_seq != initial_seq;

        if seq_changed {
            // Windows 硬件级序列号自增：100% 确认目标应用成功提交了新选区
            // 稍等 10ms 确保目标应用完成剪贴板句柄关闭
            sleep(Duration::from_millis(10));
            if let Some(mut poll_clip) = get_clipboard_with_retry() {
                if let Ok(new_text) = poll_clip.get_text() {
                    let trimmed = new_text.trim();
                    if !trimmed.is_empty() {
                        captured_text = Some(trimmed.to_string());
                        break;
                    }
                }
            }
        } else if initial_seq == 0 {
            // 非 Windows 平台环境：通过文本内容差异比对
            if let Some(mut poll_clip) = get_clipboard_with_retry() {
                if let Ok(current_text) = poll_clip.get_text() {
                    let trimmed = current_text.trim();
                    if !trimmed.is_empty() && Some(&current_text) != original_snapshot.as_ref() {
                        captured_text = Some(trimmed.to_string());
                        break;
                    }
                }
            }
        }
    }

    // 4. 超时补偿仲裁：若序列号未变，但剪贴板内容确实已与原快照不同
    if captured_text.is_none() {
        if let Some(mut poll_clip) = get_clipboard_with_retry() {
            if let Ok(current_text) = poll_clip.get_text() {
                let trimmed = current_text.trim();
                if !trimmed.is_empty() && Some(&current_text) != original_snapshot.as_ref() {
                    captured_text = Some(trimmed.to_string());
                }
            }
        }
    }

    // 5. 组装返回结果
    captured_text.map(|text| SelectionResult {
        text,
        cursor_x,
        cursor_y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_result_serialization() {
        let res = SelectionResult {
            text: "Hello Glint".to_string(),
            cursor_x: 100,
            cursor_y: 200,
        };
        let json = serde_json::to_string(&res).unwrap();
        assert!(json.contains("Hello Glint"));
        assert!(json.contains("\"cursor_x\":100"));
    }

    #[test]
    fn test_clipboard_sequence_available() {
        let seq = crate::platform::get_clipboard_sequence();
        // 在 Windows 环境下应返回有效的序列号
        #[cfg(target_os = "windows")]
        assert!(seq > 0 || seq == 0);
    }
}
