<h1 align="center"><img src="logo.svg" alt="NCMora" /></h1>

<p align="center">
  <a href="README.md">English</a>
  &nbsp;&nbsp;|&nbsp;&nbsp;
  <a href="README_zh.md">简体中文</a>
</p>

<p align="center">A fast, keyboard-driven NetEase Cloud Music client for the terminal.</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows%20%7C%20macOS-informational?logo=linux&logoColor=white" alt="Platform">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?logo=opensourceinitiative&logoColor=white" alt="License">
  <a href="https://github.com/NatsumiAi/ncmora/releases"><img src="https://img.shields.io/github/v/release/NatsumiAi/ncmora?color=32cd32&logo=github" alt="Release"></a>
</p>

## What is NCMora?

NCMora is a Rust TUI for NetEase Cloud Music. It combines QR-code, account, and phone-code login with home recommendations, search, playlists, artists, lyrics, and streaming playback. Songs and artwork are cached locally, and the embedded TMPlayer view provides a full-screen playback experience without leaving the terminal.

## Features

- QR-code, username/email, and phone verification-code login
- Automatic session restoration
- Home recommendations, daily recommended songs, and Private Radar
- Duplicate regional entries such as `欧美私人雷达` are hidden from the home page
- Playlists, albums, artists, and search
- Concurrent unfiltered search for songs, artists, and playlists with stale-request protection
- Search filters: `@single`, `@album`, `@list`, `@author`, and `@artist`
- Streaming playback with a persistent queue and optional position restore
- Single-task music downloads with configurable quality/path, cancellation, and embedded metadata
- VIP-aware audio quality selection
- Lyrics overlay with translated lyrics and NetEase YRC word-level timing
- Full-screen TMPlayer playback with smooth per-character lyric highlighting, inspired by [Pigma](https://github.com/akirco/pigma)
- Album artwork with CDN fallback URLs and `param=200y200` thumbnail requests
- Home artwork is loaded only for visible cards; artwork outside the visible area is released
- The current playback cover is deduplicated from queued tracks to reduce memory usage
- Themes, language switching, transparent backgrounds, hints, and configurable keybindings
- Built-in spectrum, real-PCM oscilloscope, vector/Lissajous, and LUFS visualizations
- Compact small-window layouts and a draggable, edge-snapping lyrics overlay
- Linux MPRIS and Windows media-control integration
- Configurable audio and artwork-cache cleanup

## Install and Run

### Pre-built releases

Download the archive for your platform from [Releases](https://github.com/NatsumiAi/ncmora/releases), extract it, and run `ncmora` (or `ncmora.exe` on Windows).

### Build from source

Install the stable Rust toolchain. Linux builds also need ALSA, D-Bus, CMake, and `pkg-config` development packages. On Debian or Ubuntu:

```bash
sudo apt update
sudo apt install -y build-essential cmake pkg-config libasound2-dev libdbus-1-dev
```

Then build and run:

```bash
cargo run --release
```

The release binary is written to `target/release/ncmora` (Windows: `target/release/ncmora.exe`). A Nerd Font is recommended for the icon glyphs used by the interface.

## First Run and Configuration

NCMora creates its files in the platform configuration directory:

- Linux: `~/.config/ncmora`
- macOS: `~/Library/Application Support/ncmora`
- Windows: `%APPDATA%\\ncmora`

The directory contains `config/default.toml`, `themes/`, and `auth/session.toml`. Audio files use the platform cache directory by default; set `cache.path` in `config/default.toml` to choose another location.

Set `NCMORA_ASSET_DIR` to use a completely custom asset root. The previous `CNMPLAYER_ASSET_DIR` and `TMPLAYER_ASSET_DIR` variables are still accepted for backward compatibility.

Useful settings include:

- Interface: `theme`, `language`, `transparent_background`, `show_hints`
- Playback: `audio_quality`, `playback_memory`, `resume_last_position`, `eq_bands_db`
- Visualization: `visualize`, `spectrum_hz`, `bars_gap`, `bar_number`
- Cache: `cache.path`, `cache.clean_strategy`, `cache.max_size_mb`, `cache.max_age_days`
- Keybindings: `keybind_*` (editable from Settings)

Missing configuration fields are added automatically when the application starts. The supported graphics protocols are `off` and `halfblocks`; older values are migrated to `halfblocks`.

## Keyboard Shortcuts

The defaults are:

| Key | Action |
| --- | --- |
| `Ctrl+S` | Open search |
| `Ctrl+F` | Enter or leave full-screen playback |
| `T` | Open Settings |
| `P` | Toggle the sidebar |
| `Q` | Quit |
| `Alt+Space` | Play or pause |
| `Alt+Left` / `Alt+Right` | Previous / next track |
| `Alt+M` | Change repeat mode |
| `Ctrl+K` | Open help |
| `Esc` | Close an overlay or go back |

On the login page, `F1`, `F2`, and `F3` select QR, account, and phone login. Search and page navigation also support `Enter`, `Tab`, arrow keys, and `Esc`.

## Development

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
```

NCMora uses Rust 2024, ratatui/crossterm for the TUI, compio/cyper for networking, ncm-api for NetEase Cloud Music APIs, rodio/symphonia for playback, and ratatui-image for artwork rendering.

## Related Projects

- [TMPlayer](https://github.com/professor-lee/TMPlayer), the embedded full-screen playback view
- [ncm-api-rs](https://github.com/imsyy/ncm-api-rs), the NetEase Cloud Music API client

## License

NCMora is licensed under [AGPL-3.0-only](LICENSE). Third-party attribution details are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md). Citation metadata is available in [CITATION.cff](CITATION.cff).

[![Star History Chart](https://api.star-history.com/image?repos=NatsumiAi/ncmora&type=date&legend=top-left)](https://www.star-history.com/?repos=NatsumiAi%2Fncmora&type=date&legend=top-left)
