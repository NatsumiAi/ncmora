use crate::data::assets;
use crate::ui::theme::{Theme, ThemePalette, detect_color_capability};
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

pub struct ThemeLoader;

#[derive(Debug, Deserialize)]
struct ThemeToml {
    /// 主题 key：优先于文件名主干（catppuccin_frappe.toml 的 key 是 frappe）。
    name: Option<String>,
    text: String,
    subtext: String,
    base: String,
    surface: String,
    buff: Option<String>,
    accent: String,
    accent2: String,
    accent3: String,
}

impl ThemeLoader {
    /// 动态加载 `themes/<key>.toml`。key 只保留 `[a-z0-9_-]`（防路径穿越），
    /// 文件缺失或格式校验不过（坏 TOML / 缺必填字段）都返回 Err，由调用方
    /// 回退默认主题。
    pub fn load(name: &str) -> Result<Theme> {
        let _ = assets::ensure_assets_ready();
        let key = sanitize_theme_key(name);
        let dir = assets::resolve_asset_path(&PathBuf::from("themes"));
        let direct = dir.join(format!("{key}.toml"));
        let path = if direct.exists() {
            direct
        } else {
            // 内置 catppuccin_* 文件名与 key 不一致：按文件内 name 字段反查，
            // 保证旧配置里的 "frappe" 等值仍能加载。
            find_theme_file_by_name_field(&dir, &key)?
        };
        let raw = fs::read_to_string(&path)?;
        let parsed: ThemeToml = toml::from_str(&raw)?;
        let buff_hex = if let Some(buff) = parsed.buff.clone() {
            buff
        } else {
            // 老主题缺 buff：推导并回写进文件（与内置主题同一升级机制）。
            let generated = derive_buff_hex(&parsed.surface);
            let upgraded = inject_buff_entry(&raw, &generated);
            let _ = fs::write(&path, upgraded);
            generated
        };

        Ok(Theme {
            name: key,
            capability: detect_color_capability(),
            palette: ThemePalette {
                text: parse_hex(&parsed.text),
                subtext: parse_hex(&parsed.subtext),
                base: parse_hex(&parsed.base),
                surface: parse_hex(&parsed.surface),
                buff: parse_hex(&buff_hex),
                accent: parse_hex(&parsed.accent),
                accent2: parse_hex(&parsed.accent2),
                accent3: parse_hex(&parsed.accent3),
            },
        })
    }

    /// 选择的主题格式有问题时回退默认主题。
    pub fn load_or_default(name: &str) -> Theme {
        Self::load(name).unwrap_or_default()
    }

    /// 扫描 `themes/*.toml`，返回通过格式校验的 key 列表（优先取文件内
    /// `name` 字段，缺省用文件名主干），按 key 排序去重；坏文件直接跳过，
    /// 不让主题循环崩掉。目录不可读时兜底 `system`。
    pub fn list_themes() -> Vec<String> {
        let _ = assets::ensure_assets_ready();
        let dir = assets::resolve_asset_path(&PathBuf::from("themes"));
        let mut keys: Vec<String> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| theme_key_for_file(&dir, &entry))
            .collect();
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            keys.push("system".to_string());
        }
        keys
    }
}

/// 单个主题文件的 key：文件须是合法主干名 + 通过格式校验；key 优先取
/// 文件内 `name` 字段（同样要求合法），否则用主干。
fn theme_key_for_file(dir: &std::path::Path, entry: &std::fs::DirEntry) -> Option<String> {
    let file = entry.file_name().to_string_lossy().into_owned();
    if !file.ends_with(".toml") {
        return None;
    }
    let stem = &file[..file.len() - 5];
    if !is_valid_theme_key(stem) {
        return None;
    }
    let raw = fs::read_to_string(dir.join(&file)).ok()?;
    let parsed = toml::from_str::<ThemeToml>(&raw).ok()?;
    match parsed.name {
        Some(name) => {
            let key = sanitize_theme_key(&name);
            is_valid_theme_key(&key).then_some(key)
        }
        None => Some(stem.to_string()),
    }
}

/// 按 `name` 字段反查主题文件（catppuccin_* 这类文件名与 key 不一致时）。
fn find_theme_file_by_name_field(dir: &std::path::Path, key: &str) -> Result<PathBuf> {
    let hit = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().is_some_and(|ext| ext == "toml")
                && fs::read_to_string(path).is_ok_and(|raw| {
                    toml::from_str::<ThemeToml>(&raw).is_ok_and(|parsed| {
                        parsed.name.is_some_and(|n| sanitize_theme_key(&n) == key)
                    })
                })
        });
    hit.ok_or_else(|| anyhow::anyhow!("theme not found: {key}"))
}

/// key 归一化：小写并剔除 `[a-z0-9_-]` 之外的字符（空 key 落到不存在的
/// `themes/.toml`，加载失败后回退默认主题）。
fn sanitize_theme_key(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-')
        .collect()
}

fn is_valid_theme_key(stem: &str) -> bool {
    !stem.is_empty()
        && stem
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn parse_hex(raw: &str) -> (u8, u8, u8) {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return (255, 255, 255);
    }

    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    (r, g, b)
}

fn derive_buff_hex(surface_hex: &str) -> String {
    let (r, g, b) = parse_hex(surface_hex);
    format!(
        "#{:02X}{:02X}{:02X}",
        r.saturating_add(10),
        g.saturating_add(10),
        b.saturating_add(10)
    )
}

fn inject_buff_entry(raw: &str, buff_hex: &str) -> String {
    if raw.lines().any(|line| is_toml_key(line, "buff")) {
        return raw.to_string();
    }

    let mut out = String::with_capacity(raw.len() + 24);
    let mut inserted = false;

    for line in raw.lines() {
        out.push_str(line);
        out.push('\n');
        if !inserted && is_toml_key(line, "surface") {
            out.push_str(&format!("buff = \"{}\"\n", buff_hex));
            inserted = true;
        }
    }

    if !inserted {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!("buff = \"{}\"\n", buff_hex));
    }

    out
}

fn is_toml_key(line: &str, key: &str) -> bool {
    let trimmed = line.trim_start();
    if !trimmed.starts_with(key) {
        return false;
    }
    trimmed[key.len()..].trim_start().starts_with('=')
}
