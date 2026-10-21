use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// UI-agnostic application configuration.
///
/// The shell (Tauri, Slint, CLI) owns the config directory decision and passes
/// an explicit path in. `default_config_path()` is only a convenience for
/// headless tools and tests.
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
            api_key: String::new(),
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

/// XDG config path used by the CLI and tests.
///
/// The Tauri shell should keep using its own `app_config_dir()` and call
/// `load_config(path)` / `save_config(path, cfg)` directly.
pub fn default_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.trim().is_empty() {
            return PathBuf::from(xdg).join("glint").join("config.json");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home).join(".config").join("glint").join("config.json");
        }
    }
    PathBuf::from("glint_config.json")
}

pub fn load_config(path: &Path) -> AppConfig {
    match fs::read_to_string(path) {
        Ok(content) => serde_json::from_str::<AppConfig>(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(path: &Path, config: &AppConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())
}

pub fn load_default_config() -> AppConfig {
    load_config(&default_config_path())
}

pub fn save_default_config(config: &AppConfig) -> Result<(), String> {
    save_config(&default_config_path(), config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_stable() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.model, "deepseek-chat");
        assert_eq!(cfg.hotkey_trigger, "Alt+Q");
        assert_eq!(cfg.theme, "dark");
    }

    #[test]
    fn config_round_trips_through_json() {
        let cfg = AppConfig::default();
        let serialized = serde_json::to_string(&cfg).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(cfg, deserialized);
    }

    #[test]
    fn config_round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("glint-config-test-{}", std::process::id()));
        let path = dir.join("config.json");
        let mut cfg = AppConfig::default();
        cfg.model = "test-model".to_string();
        save_config(&path, &cfg).unwrap();
        assert_eq!(load_config(&path), cfg);
        let _ = fs::remove_dir_all(dir);
    }
}
