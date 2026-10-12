use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub hotkey_trigger: String,
    pub hotkey_toggle: String,
    pub icon_plan: String,
    pub theme: String,
    pub always_on_top: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.deepseek.com/v1".to_string(),
            api_key: "".to_string(),
            model: "deepseek-chat".to_string(),
            temperature: 0.3,
            hotkey_trigger: "Alt+Q".to_string(),
            hotkey_toggle: "Alt+D".to_string(),
            icon_plan: "A".to_string(),
            theme: "dark".to_string(),
            always_on_top: true,
        }
    }
}

fn get_config_path(app: &AppHandle) -> PathBuf {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let _ = fs::create_dir_all(&config_dir);
        config_dir.join("config.json")
    } else {
        PathBuf::from("glint_config.json")
    }
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = get_config_path(app);
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(cfg) = serde_json::from_str::<AppConfig>(&content) {
            return cfg;
        }
    }
    AppConfig::default()
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = get_config_path(app);
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.model, "deepseek-chat");
        assert_eq!(cfg.hotkey_trigger, "Alt+Q");
        assert_eq!(cfg.icon_plan, "A");
    }

    #[test]
    fn test_config_serialization() {
        let cfg = AppConfig::default();
        let serialized = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn test_config_custom_icon_plan() {
        let mut cfg = AppConfig::default();
        cfg.icon_plan = "B".to_string();
        cfg.always_on_top = false;
        let serialized = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.icon_plan, "B");
        assert!(!deserialized.always_on_top);
    }
}
