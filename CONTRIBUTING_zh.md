<p align="center">
	<a href="CONTRIBUTING.md">English</a>
	&nbsp;&nbsp;&nbsp;|&nbsp;&nbsp;&nbsp;
	<a href="CONTRIBUTING_zh.md">简体中文</a>
</p>

# 为 CNMPlayer 贡献代码

CNMPlayer 是一个运行在终端中的网易云音乐客户端（Rust + TUI）。本文档说明分支与 PR 规则、开发环境、仓库布局、格式与 CI 约定，以及终端界面特有的额外要求。

## 分支与 PR 规则

- **所有 PR 一律向 `develop` 分支发起。** `develop` 是集成分支，不要直接向 `main` 发起 PR。
- **`main` 是发布分支，仅由仓库作者维护**：由作者本人从 `develop` 向 `main` 发起 PR，请不要自行向 `main` 提交改动。
- **一个 PR 只处理一个主题。** 面向 `develop` 的每个 PR 只能处理一个修改主题；有多个互不相关的修改时，请拆成多个 PR —— 混合多个主题的 PR 将被拒绝。重大贡献除外，由作者判断。

## 开发环境

Rust：stable 工具链。根 crate（`cnmplayer`）使用 edition 2024，vendored 的 `ncm-api` crate 使用 edition 2021。

系统构建依赖 —— 与 CI 在 `ubuntu-24.04` 上安装的列表一致：

```bash
# Debian / Ubuntu
sudo apt update
sudo apt install -y build-essential cmake pkg-config \
  libasound2-dev libchafa-dev libpipewire-0.3-dev libssl-dev libglib2.0-dev libclang-dev
```

```bash
# Arch Linux
sudo pacman -S --needed base-devel cmake pkg-config alsa-lib chafa pipewire openssl glib2 clang
```

- `libchafa-dev` 要求 chafa ≥ 1.8.0 —— 图像渲染通过 `pkg-config` 探测它；
- `libclang-dev` 与 `libpipewire-0.3-dev` 是 PipeWire 音频后端在构建期生成绑定所需；
- `libasound2-dev` / `alsa-lib` **只在构建期需要**：`cpal` 在 Linux 上无条件编译其 ALSA 后端，缺了这个开发包就构建不出来；但播放走 PipeWire，ALSA 路径不属于受支持的运行配置。

运行要求：Linux 上的 PipeWire 音频（ALSA 后端已弃用，不要按它测试或写文档），外加运行时的 chafa 共享库；可选的 `cava` 可执行文件用于 `bars` 频谱（没有它时默认可视化会变成示波器）；强烈建议使用 Nerd Font —— 否则部分图标会显示为缺字方块。

常用命令：

```bash
cargo run                            # 开发构建
cargo build --release                # release 构建
cargo test                           # 单元测试
cargo check --locked --all-targets   # CI 在 PR 上执行的内容
```

开发时有用的环境变量：`CNMPLAYER_ASSET_DIR`（资产根目录，默认 `~/.config/cnmplayer`）和 `TMPLAYER_CAVA`（显式指定 cava 可执行文件）。

Cargo feature：`default = ["easter-egg"]` —— About 弹窗里的形象彩蛋，用 `--no-default-features` 可剔除。

## 仓库布局

| 路径 | 内容 |
| --- | --- |
| `src/main.rs` | 入口：终端初始化、资产根目录、交给主程序。 |
| `src/app/` | 主程序核心 —— `mod.rs`（状态与逻辑）、`startup.rs`（启动链）、`api.rs`、`streaming.rs`（流式播放与缓存）、`player.rs`（播放）、`mpris_bridge.rs`。 |
| `src/ui/` | 主程序界面 —— 每个页面或面板一个模块：`login.rs`、`home.rs`、`playlist.rs`、`author.rs`、`search.rs`、`search_box.rs`、`settings.rs`、`player_bar.rs`、`page_lyrics.rs`、`small_window.rs`、`loading.rs`、`theme.rs`。 |
| `src/data/` | 配置与持久化 —— `config.rs`、`assets.rs`、`session.rs`、`playback_session.rs`、`private_roam.rs`、`theme_loader.rs`。 |
| `src/render/` | 封面与图形渲染，以及彩蛋形象的帧数据。 |
| `src/tmplayer/` | 内置全屏播放页（TMPlayer），自成一套：`app/`、`ui/`、`audio/`（cava 对接、PCM 抽头、LUFS 计量）、`render/`（频谱、示波器）、`playback/`、`data/`、`utils/`。 |
| `ncm-api-rs/` | 以 path 依赖形式 vendored 的 `ncm-api` crate，来自 [imsyy/ncm-api-rs](https://github.com/imsyy/ncm-api-rs)；自带 `rustfmt.toml` / `clippy.toml` 与 `docs/API.md`。 |
| `config/default.toml` | 默认配置模板。启动时程序会据此在资产根目录下写入并修复 `config/default.toml`。 |
| `themes/` | TOML 配色主题：`system`、`latte`、`frappe`、`macchiato`、`mocha`。 |
| `about/` | About 弹窗的内容 —— 链接、QQ 群、点阵图案。 |
| `assets/`、`tools/` | 彩蛋形象源图，以及重新生成其点阵帧的脚本。 |
| `.github/` | `workflows/ci.yml`、`workflows/release.yml` 与 `scripts/aur_sync.sh`。 |
| 根目录文档 | `README.md` / `README_zh.md`（中英两份并行维护）、`CITATION.cff`、`THIRD_PARTY_NOTICES.md`、`LICENSE`（AGPL-3.0-only）。 |

## 格式与 CI

格式化工具是 rustfmt，仓库自带配置：

- 根目录 `.rustfmt.toml`：`max_width = 100`、`edition = "2024"`；
- `ncm-api-rs/rustfmt.toml`：`edition = "2021"`、`max_width = 100`、`use_field_init_shorthand = true` —— 动过 vendored crate 时一并执行 `cargo fmt --manifest-path ncm-api-rs/Cargo.toml`。

Clippy：`cargo clippy --all-targets`；`ncm-api-rs/clippy.toml` 把 `too-many-arguments-threshold` 放宽到 8、`type-complexity-threshold` 放宽到 300。

CI（`.github/workflows/ci.yml`）在面向 `main` / `develop` 的 PR 以及推送到 `develop` 时触发：在 `ubuntu-24.04` 上安装上面列出的系统依赖，使用 stable 工具链，只跑一条命令：

```bash
cargo check --locked --all-targets
```

它**不跑** `cargo fmt`、`cargo clippy` 和 `cargo test`，请在发 PR 之前自行跑完。

版本号与打包不属于普通 PR 的范围：发布由 `v*` tag 触发，tag 必须与 `Cargo.toml` 里的版本号一致，`release.yml` 会构建 amd64/aarch64 两个压缩包、创建 GitHub Release，并同步 AUR 包。

## TUI 专属要求

终端界面不能只满足于「在我机器上是好的」。

- **键盘是主要交互方式。** 设计以键盘操作优先：任何功能都必须仅靠键盘就能到达并操作，两者冲突时以键盘路径为准。鼠标交互只是叠加在上层的附加功能，用于非必要功能，绝不能成为到达某个功能的唯一途径。
- **基本功能必须能在纯 TTY 下可用。** 以 `kmscon` 为基准环境：没有模拟器专属转义序列，没有 Nerd Font，鼠标上报甚至可能完全不可用。登录、浏览、播放控制与退出都必须在那里保持可用。
- **渲染类改动要附渲染证据。** 任何改变「画出来的东西」的改动 —— 布局、动画、封面、可视化、配色 —— 都要在 PR 里附截图或简短录屏。
- **说明你的运行环境**：终端模拟器（名称与版本）、所用字体、终端尺寸、是否装了 Nerd Font。界面多处绘制图标字形，渲染问题大多与终端相关；若改动涉及输入处理、按键或快捷键，还要在纯 TTY（`kmscon`）里过一遍 —— 键盘优先的底线在那里必须成立。
- **覆盖小窗口阈值。** 主程序内容页低于 README「小窗口模式」一节给出的阈值时会切换为扁平布局；请在边界尺寸上验证，包括 `Terminal too small` 的情况。
- **改动到的控件要实测键盘路径，有鼠标目标的一并实测鼠标路径**：折叠播放栏的上一首 / 播放暂停 / 下一首、收藏与循环模式按钮，以及进度条都可点击；侧边栏支持滚轮、单击与双击（400 ms 判定窗口）。
- **新增或修改界面文案时，中英两种界面语言都要检查**（`language = "zh"` / `"en"`）。
- **涉及 cava 的改动要在装与不装 cava 两种情况下验证。** 若改动了频谱相关代码：没有 cava 时 `bars` 不可用，默认可视化会变成示波器。
