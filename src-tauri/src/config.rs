use std::fs;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

pub use glint_core::AppConfig;

fn get_config_path(app: &AppHandle) -> PathBuf {
    if let Ok(config_dir) = app.path().app_config_dir() {
        let _ = fs::create_dir_all(&config_dir);
        config_dir.join("config.json")
    } else {
        PathBuf::from("glint_config.json")
    }
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    glint_core::load_config(&get_config_path(app))
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    glint_core::save_config(&get_config_path(app), config)
}
