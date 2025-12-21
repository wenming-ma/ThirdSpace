use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub api_key: String,
    pub model: String,
    pub target_language: String,
    pub reasoning_enabled: bool,
    pub hotkey: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: "google/gemini-3-flash-preview".to_string(),
            target_language: "English".to_string(),
            reasoning_enabled: true,
            hotkey: "Ctrl+Alt+T".to_string(),
        }
    }
}

pub fn app_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().ok_or_else(|| anyhow!("Could not determine home directory"))?;
    Ok(home_dir.join(".thirdspace"))
}

pub fn logs_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("logs"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(app_dir()?.join("config.json"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let data = fs::read_to_string(&path).context("read config.json")?;
    let config: Config = serde_json::from_str(&data).context("parse config.json")?;
    Ok(config)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create config directory")?;
    }
    let data = serde_json::to_string_pretty(config).context("serialize config")?;
    fs::write(&path, data).context("write config.json")?;
    Ok(())
}

pub fn migrate_legacy_data() -> Result<()> {
    let new_base = app_dir()?;
    fs::create_dir_all(&new_base).context("create new data directory")?;

    if let Some(old_config_dir) = dirs::config_dir().map(|dir| dir.join("ThirdSpace")) {
        let old_config_path = old_config_dir.join("config.json");
        let new_config_path = new_base.join("config.json");
        if old_config_path.exists() {
            move_path(&old_config_path, &new_config_path)
                .context("migrate legacy config")?;
        }
        let _ = fs::remove_dir_all(&old_config_dir);
    }

    if let Some(old_data_dir) = dirs::data_local_dir().map(|dir| dir.join("ThirdSpace")) {
        let old_logs_dir = old_data_dir.join("logs");
        let new_logs_dir = new_base.join("logs");
        merge_dir(&old_logs_dir, &new_logs_dir).context("migrate legacy logs")?;
        let _ = fs::remove_dir_all(&old_data_dir);
    }

    Ok(())
}

fn merge_dir(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if !target.exists() {
        return move_path(source, target);
    }
    fs::create_dir_all(target).context("create target directory")?;
    for entry in fs::read_dir(source).context("read source directory")? {
        let entry = entry.context("read source entry")?;
        let path = entry.path();
        let target_path = target.join(entry.file_name());
        if path.is_dir() {
            merge_dir(&path, &target_path)?;
        } else {
            let final_target = if target_path.exists() {
                unique_path(&target_path)
            } else {
                target_path
            };
            move_path(&path, &final_target)?;
        }
    }
    let _ = fs::remove_dir_all(source);
    Ok(())
}

fn move_path(source: &Path, target: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let final_target = if target.exists() {
        unique_path(target)
    } else {
        target.to_path_buf()
    };
    if let Some(parent) = final_target.parent() {
        fs::create_dir_all(parent).context("create target parent")?;
    }
    if fs::rename(source, &final_target).is_ok() {
        return Ok(());
    }
    if source.is_dir() {
        copy_dir_recursive(source, &final_target)?;
        fs::remove_dir_all(source).context("remove source directory")?;
    } else {
        fs::copy(source, &final_target).context("copy source file")?;
        fs::remove_file(source).context("remove source file")?;
    }
    Ok(())
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).context("create target directory")?;
    for entry in fs::read_dir(source).context("read source directory")? {
        let entry = entry.context("read source entry")?;
        let path = entry.path();
        let target_path = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(&path, &target_path)?;
        } else {
            fs::copy(&path, &target_path).context("copy file")?;
        }
    }
    Ok(())
}

fn unique_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }
    let file_name = match path.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => return path.to_path_buf(),
    };
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let mut candidate = parent.join(format!("{}.legacy", file_name));
    if !candidate.exists() {
        return candidate;
    }
    for idx in 1..1000 {
        candidate = parent.join(format!("{}.legacy-{}", file_name, idx));
        if !candidate.exists() {
            return candidate;
        }
    }
    candidate
}
