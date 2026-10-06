use crate::data::{assets, atomic_file};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize, Default)]
struct SessionRecord {
    cookie: String,
    updated_at: i64,
}

pub fn load_cookie() -> Result<Option<String>> {
    let path = session_path();
    if !path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    // 损坏的会话只代表本地登录状态失效：回到登录页，由下一次登录覆盖它。
    // 文件读取权限/IO 错误仍然传播，避免把系统故障伪装成未登录。
    let record: SessionRecord = toml::from_str(&raw).unwrap_or_default();
    let cookie = record.cookie.trim().to_string();
    if cookie.is_empty() {
        return Ok(None);
    }

    Ok(Some(cookie))
}

pub fn save_cookie(cookie: &str) -> Result<()> {
    let path = session_path();
    let record = SessionRecord {
        cookie: cookie.to_string(),
        updated_at: now_unix(),
    };
    let raw = toml::to_string_pretty(&record).context("serialize session cookie")?;
    atomic_file::write_atomic_with_mode(&path, raw.as_bytes(), Some(0o600))
}

pub fn clear_cookie() -> Result<()> {
    let path = session_path();
    if path.is_file() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn session_path() -> PathBuf {
    assets::resolve_asset_path(Path::new("auth/session.toml"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}
