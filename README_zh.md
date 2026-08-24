<h1 align="center"><img src="logo.svg" alt="NCMora" /></h1>

<p align="center">
  <a href="README.md">English</a>
  &nbsp;&nbsp;|&nbsp;&nbsp;
  <a href="README_zh.md">简体中文</a>
</p>

<p align="center">快速、键盘驱动的网易云音乐终端客户端。</p>

<p align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/Platform-Linux%20%7C%20Windows%20%7C%20macOS-informational?logo=linux&logoColor=white" alt="Platform">
  <img src="https://img.shields.io/badge/License-AGPL--3.0-blue?logo=opensourceinitiative&logoColor=white" alt="License">
  <a href="https://github.com/NatsumiAi/ncmora/releases"><img src="https://img.shields.io/github/v/release/NatsumiAi/ncmora?color=32cd32&logo=github" alt="Release"></a>
</p>

## 项目简介

NCMora 是一个使用 Rust 编写的网易云音乐 TUI 客户端。它支持二维码、账号和手机验证码登录，提供首页推荐、搜索、歌单、专辑、歌手、歌词和流式播放功能。歌曲和封面会缓存到本地，内置的 TMPlayer 页面可以在终端中提供全屏播放体验。

## 主要功能

- 二维码、用户名/邮箱、手机验证码登录
- 自动恢复上次登录会话
- 首页推荐、每日推荐歌曲和私人雷达
- 首页会隐藏 `欧美私人雷达` 等重复的地区推荐入口
- 歌单、专辑、歌手和搜索
- 搜索过滤器：`@single`、`@album`、`@list`、`@author`、`@artist`
- 流式播放、播放队列记忆和可选的播放位置恢复
- 根据 VIP 权限自动限制音质选项
- 歌词浮层、翻译歌词和网易云 YRC 逐字时间轴
- TMPlayer 全屏播放，以及参考 [Pigma](https://github.com/akirco/pigma) 实现的歌词逐字高亮播放效果
- 专辑封面显示，支持网易云 CDN 备用地址和 `param=200y200` 缩略图请求
- 首页只加载可视卡片的封面，离开可视区域后释放封面数据
- 播放当前封面与队列中的重复数据会被去重，降低内存占用
- 主题、语言、透明背景、提示开关和可配置快捷键
- 内置频谱和示波器可视化
- Linux MPRIS 与 Windows 媒体控制支持
- 可配置的音频与封面缓存清理策略

## 安装与运行

### 下载发行版

从 [Releases](https://github.com/NatsumiAi/ncmora/releases) 下载对应平台的压缩包，解压后运行 `ncmora`（Windows 下为 `ncmora.exe`）。

### 从源码构建

请先安装 stable Rust 工具链。Linux 还需要 ALSA、D-Bus、CMake 和 `pkg-config` 开发依赖。在 Debian 或 Ubuntu 上可以执行：

```bash
sudo apt update
sudo apt install -y build-essential cmake pkg-config libasound2-dev libdbus-1-dev
```

构建并运行：

```bash
cargo run --release
```

发布版本位于 `target/release/ncmora`（Windows：`target/release/ncmora.exe`）。界面使用了一些图标字符，建议使用 Nerd Font 字体。

## 首次运行与配置

NCMora 会在系统配置目录中创建文件：

- Linux：`~/.config/ncmora`
- macOS：`~/Library/Application Support/ncmora`
- Windows：`%APPDATA%\\ncmora`

目录中包含 `config/default.toml`、`themes/` 和 `auth/session.toml`。音频默认使用系统缓存目录；也可以在 `config/default.toml` 中通过 `cache.path` 指定位置。

设置 `NCMORA_ASSET_DIR` 可以使用自定义资源目录。为兼容旧版本，`CNMPLAYER_ASSET_DIR` 和 `TMPLAYER_ASSET_DIR` 仍然有效。

常用配置包括：

- 界面：`theme`、`language`、`transparent_background`、`show_hints`
- 播放：`audio_quality`、`playback_memory`、`resume_last_position`、`eq_bands_db`
- 可视化：`visualize`、`spectrum_hz`、`bars_gap`、`bar_number`
- 缓存：`cache.path`、`cache.clean_strategy`、`cache.max_size_mb`、`cache.max_age_days`
- 快捷键：`keybind_*`（可在设置页面修改）

程序启动时会自动补齐缺失的配置字段。目前图像协议支持 `off` 和 `halfblocks`，旧版本中的其他值会自动迁移为 `halfblocks`。

## 快捷键

默认快捷键如下：

| 按键 | 操作 |
| --- | --- |
| `Ctrl+S` | 打开搜索 |
| `Ctrl+F` | 进入或退出全屏播放 |
| `T` | 打开设置 |
| `P` | 展开或收起侧栏 |
| `Q` | 退出 |
| `Alt+Space` | 播放或暂停 |
| `Alt+Left` / `Alt+Right` | 上一首 / 下一首 |
| `Alt+M` | 切换循环模式 |
| `Ctrl+K` | 打开帮助 |
| `Esc` | 关闭浮层或返回 |

登录页面使用 `F1`、`F2`、`F3` 选择二维码、账号和手机登录。搜索与页面导航支持 `Enter`、`Tab`、方向键和 `Esc`。

## 开发检查

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
```

项目使用 Rust 2024、ratatui/crossterm、compio/cyper、ncm-api、rodio/symphonia 和 ratatui-image。

## 相关项目

- [TMPlayer](https://github.com/professor-lee/TMPlayer)：内置的全屏播放页面
- [ncm-api-rs](https://github.com/imsyy/ncm-api-rs)：网易云音乐 API 客户端

## 许可证

NCMora 使用 [AGPL-3.0-only](LICENSE) 许可证。第三方归属信息见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)，引用信息见 [CITATION.cff](CITATION.cff)。

[![Star History Chart](https://api.star-history.com/image?repos=NatsumiAi/ncmora&type=date&legend=top-left)](https://www.star-history.com/?repos=NatsumiAi%2Fncmora&type=date&legend=top-left)
