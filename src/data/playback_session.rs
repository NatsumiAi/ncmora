use crate::data::assets;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaybackSessionTrack {
    pub song_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_ms: i64,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PlaybackSessionRecord {
    #[serde(default)]
    pub queue: Vec<PlaybackSessionTrack>,
    pub current_index: Option<usize>,
    /// Last known position of the active track, in milliseconds.
    #[serde(default)]
    pub position_ms: Option<u64>,
    pub repeat_mode: Option<String>,
    pub updated_at: i64,
}

pub fn load() -> Result<Option<PlaybackSessionRecord>> {
    let path = session_path();
    if !path.is_file() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)?;
    let record: PlaybackSessionRecord = toml::from_str(&raw).unwrap_or_default();
    if record.queue.is_empty() {
        return Ok(None);
    }

    Ok(Some(record))
}

pub fn save(record: &PlaybackSessionRecord) -> Result<()> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
    }

    let mut payload = record.clone();
    payload.updated_at = now_unix();
    let raw = toml::to_string_pretty(&payload).unwrap_or_default();
    fs::write(&path, raw).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

pub fn clear() -> Result<()> {
    let path = session_path();
    if path.is_file() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

fn session_path() -> PathBuf {
    assets::resolve_asset_path(Path::new("playback/session.toml"))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::PlaybackSessionRecord;

    #[test]
    fn old_sessions_without_position_remain_loadable() {
        let record: PlaybackSessionRecord = toml::from_str(
            r#"
                queue = []
                current_index = 0
                repeat_mode = "sequence"
                updated_at = 0
            "#,
        )
        .expect("legacy playback session should deserialize");

        assert_eq!(record.position_ms, None);
    }

    #[test]
    fn position_round_trips_in_session_format() {
        let record = PlaybackSessionRecord {
            position_ms: Some(12_345),
            ..PlaybackSessionRecord::default()
        };
        let encoded = toml::to_string(&record).expect("session should serialize");
        let decoded: PlaybackSessionRecord =
            toml::from_str(&encoded).expect("session should deserialize");

        assert_eq!(decoded.position_ms, Some(12_345));
    }
}
