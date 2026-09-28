use crate::app::{App, SearchItemKind, SearchState};
use crate::data::config::Language;
use crate::ui::page_lyrics;
use crate::ui::player_bar;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// 作者卡片：上边框 + 两行内容（头像跨两行）+ 下边框。
const ARTIST_CARD_HEIGHT: u16 = 4;
/// 卡片头像区：2 行高、4 列宽，约等于方形。
const ARTIST_CARD_AVATAR_HEIGHT: u16 = 2;
const ARTIST_CARD_AVATAR_WIDTH: u16 = 4;
/// 卡片内头像起始列（边框 + 2 空格）与文字起始列（头像后再 3 空格）。
const ARTIST_CARD_AVATAR_X: u16 = 3;
const ARTIST_CARD_TEXT_X: u16 = 10;
/// 面板比这还窄 / 矮就用单行样式，避免卡片被压扁。
const ARTIST_CARD_MIN_WIDTH: u16 = 24;

pub fn draw_search(frame: &mut Frame, app: &mut App) {
    app.clear_player_bar_hits();
    app.clear_content_hits();

    let size = frame.area();
    frame.render_widget(Block::default().style(base_bg_style(app)), size);

    if !app.config.small_window_display && (size.width < 42 || size.height < 14) {
        frame.render_widget(
            Paragraph::new(match app.config.language {
                Language::Zh => "终端窗口过小",
                Language::En => "Terminal too small",
            })
            .style(Style::default().fg(app.theme.color_subtext())),
            size,
        );
        return;
    }

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(player_bar::PLAYER_BAR_HEIGHT),
        ])
        .split(size);

    draw_result_panel(frame, app, rows[0]);
    if app.config.page_lyrics {
        page_lyrics::draw_page_lyrics_overlay(frame, app, rows[0]);
    }

    player_bar::draw_collapsed_player_bar(frame, app, rows[1]);
}

fn draw_result_panel(frame: &mut Frame, app: &mut App, area: Rect) {
    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.width < 10 || inner.height < 2 {
        return;
    }

    let list_height = if app.config.show_hints {
        inner.height.saturating_sub(1)
    } else {
        inner.height
    };
    if list_height == 0 {
        return;
    }

    let list_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: list_height,
    };
    let hint_rect = Rect {
        x: inner.x,
        y: inner.y + list_height,
        width: inner.width,
        height: 1,
    };

    let card = list_area.width >= ARTIST_CARD_MIN_WIDTH && list_area.height >= ARTIST_CARD_HEIGHT;

    // 条目高度不一（作者卡片 4 行），可见条目数按实际高度累计；先按上一帧的滚动起点
    // 估算一次，交给状态机修正焦距后再算最终窗口，与 `ensure_focus_visible` 口径一致。
    let first = app.search.effective_scroll_offset();
    let end = visible_search_end(&app.search, first, list_area.height, card);
    app.search
        .set_visible_rows(end.saturating_sub(first).max(1));
    let first = app.search.effective_scroll_offset();
    let end = visible_search_end(&app.search, first, list_area.height, card);

    let mut y = list_area.y;
    for item_idx in first..end {
        let kind = app.search.results[item_idx].kind;
        // 分隔线只做视觉区分，不切分滚动区域：跨分区滚动与旧版一致。
        if item_idx > 0 && app.search.results[item_idx - 1].kind != kind {
            draw_search_divider(
                frame,
                app,
                Rect {
                    x: list_area.x,
                    y,
                    width: list_area.width,
                    height: 1,
                },
            );
            y += 1;
        }

        let height = item_height(kind, card);
        let row = Rect {
            x: list_area.x,
            y,
            width: list_area.width,
            height,
        };

        app.push_search_item_hit(
            crate::app::HitRect {
                x: row.x,
                y: row.y,
                width: row.width,
                height: row.height,
            },
            item_idx,
        );

        let focused = item_idx == app.search.focused_idx;
        if card && kind == SearchItemKind::Artist {
            render_artist_card(frame, app, row, item_idx, focused);
        } else {
            let ordinal = search_item_ordinal(&app.search, item_idx);
            render_search_row(frame, app, row, item_idx, ordinal, focused);
        }
        y += height;
    }

    if app.config.show_hints && list_height < inner.height {
        let hint = match app.config.language {
            Language::Zh => {
                "Enter 打开/播放  Esc 返回  无后缀=作者/歌单/单曲  @single/@album/@author/@list 限定类型  @author 空关键词=关注作者"
            }
            Language::En => {
                "Enter open/play  Esc back  no suffix = artists/playlists/songs  @single/@album/@author/@list narrows  bare @author = followed"
            }
        };
        frame.render_widget(
            Paragraph::new(hint).style(Style::default().fg(app.theme.color_subtext())),
            hint_rect,
        );
    }
}

/// 条目占用的行数（不含它前面的分隔线）。
fn item_height(kind: SearchItemKind, card: bool) -> u16 {
    if card && kind == SearchItemKind::Artist {
        ARTIST_CARD_HEIGHT
    } else {
        1
    }
}

/// 条目自身 + 它前面的分区线占用的行数。
fn search_row_height(state: &SearchState, index: usize, card: bool) -> u16 {
    let divider =
        u16::from(index > 0 && state.results[index - 1].kind != state.results[index].kind);
    divider + item_height(state.results[index].kind, card)
}

/// 从 `first` 起、在 `height` 行内**完整**放得下的条目区间上界（不含）。
/// 极矮面板至少保留一条，避免焦点落在看不见的位置。
fn visible_search_end(state: &SearchState, first: usize, height: u16, card: bool) -> usize {
    let mut used = 0u16;
    let mut end = first;
    while end < state.results.len() {
        let needed = search_row_height(state, end, card);
        if used.saturating_add(needed) > height {
            break;
        }
        used += needed;
        end += 1;
    }

    if end == first && first < state.results.len() {
        end = first + 1;
    }
    end
}

/// 分区内序号（同一 kind 内的第几条）。带后缀搜索只有一种 kind，等价于旧版的行号。
fn search_item_ordinal(state: &SearchState, index: usize) -> usize {
    let kind = state.results[index].kind;
    state.results[..index]
        .iter()
        .filter(|item| item.kind == kind)
        .count()
        + 1
}

fn draw_search_divider(frame: &mut Frame, app: &App, row: Rect) {
    if row.width == 0 || row.height == 0 {
        return;
    }

    frame.render_widget(
        Paragraph::new("─".repeat(usize::from(row.width)))
            .style(Style::default().fg(app.theme.color_subtext())),
        row,
    );
}

/// 作者条目：整行宽的卡片，左侧头像走封面管线，名字与类型标签分列两行。
fn render_artist_card(frame: &mut Frame, app: &mut App, row: Rect, item_idx: usize, focused: bool) {
    let name = app.search.results[item_idx].left_label.clone();
    let tag = app.search.results[item_idx]
        .kind
        .tag()
        .unwrap_or_default()
        .to_string();

    let border_style = if focused {
        Style::default()
            .fg(app.theme.color_accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.color_surface())
    };
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(base_bg_style(app)),
        row,
    );

    let avatar = Rect {
        x: row.x.saturating_add(ARTIST_CARD_AVATAR_X),
        y: row.y.saturating_add(1),
        width: ARTIST_CARD_AVATAR_WIDTH.min(row.width),
        height: ARTIST_CARD_AVATAR_HEIGHT.min(row.height),
    };
    if !avatar.is_empty() {
        let draw_ascii = app.draw_ascii();
        let text_style = Style::default().fg(app.theme.color_text());
        app.search.results[item_idx].cover.render(
            frame,
            &mut app.graphics_picker,
            avatar,
            text_style,
            None,
            draw_ascii,
        );
    }

    let text_width = row
        .width
        .saturating_sub(ARTIST_CARD_TEXT_X.saturating_add(2));
    if text_width == 0 {
        return;
    }

    let name_style = if focused {
        Style::default()
            .fg(app.theme.color_accent2())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(app.theme.color_text())
    };
    frame.render_widget(
        Paragraph::new(clip_to_display_width(&name, usize::from(text_width))).style(name_style),
        Rect {
            x: row.x.saturating_add(ARTIST_CARD_TEXT_X),
            y: row.y.saturating_add(1),
            width: text_width,
            height: 1,
        },
    );

    let tag_style = if focused {
        Style::default().fg(app.theme.color_accent())
    } else {
        Style::default().fg(app.theme.color_subtext())
    };
    frame.render_widget(
        Paragraph::new(tag)
            .style(tag_style)
            .alignment(Alignment::Right),
        Rect {
            x: row.x.saturating_add(ARTIST_CARD_TEXT_X),
            y: row.y.saturating_add(2),
            width: text_width,
            height: 1,
        },
    );
}

fn render_search_row(
    frame: &mut Frame,
    app: &App,
    row: Rect,
    item_idx: usize,
    ordinal: usize,
    focused: bool,
) {
    let item = &app.search.results[item_idx];
    let is_now_playing = app.is_now_playing_song(item.song_id.as_deref());
    let zebra_bg = if app.config.transparent_background {
        None
    } else if item_idx.is_multiple_of(2) {
        Some(app.theme.color_base())
    } else {
        Some(app.theme.color_surface())
    };

    let row_style = if focused {
        Style::default()
            .fg(app.theme.color_base())
            .bg(app.theme.color_accent())
            .add_modifier(Modifier::BOLD)
    } else {
        let mut style = Style::default().fg(if is_now_playing {
            app.theme.color_accent3()
        } else {
            app.theme.color_text()
        });
        if is_now_playing {
            style = style.add_modifier(Modifier::BOLD);
        }
        if let Some(bg) = zebra_bg {
            style = style.bg(bg);
        }
        style
    };

    let right = item
        .kind
        .tag()
        .map(str::to_string)
        .unwrap_or_else(|| item.right_label.clone());

    let left = format!("{:02}. {}", ordinal, item.left_label);
    let reserved = display_width(&right) + 1;
    let left_max = usize::from(row.width).saturating_sub(reserved);
    let clipped_left = clip_to_display_width(&left, left_max);
    let used = display_width(&clipped_left) + display_width(&right);
    let space = usize::from(row.width).saturating_sub(used).max(1);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(clipped_left, row_style),
            Span::styled(" ".repeat(space), row_style),
            Span::styled(right, row_style),
        ])),
        row,
    );
}

fn base_bg_style(app: &App) -> Style {
    if app.config.transparent_background {
        Style::default()
    } else {
        Style::default().bg(app.theme.color_base())
    }
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn clip_to_display_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > max_width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CoverFetchState;

    fn item(kind: SearchItemKind, label: &str) -> crate::app::SearchItem {
        crate::app::SearchItem {
            kind,
            left_label: label.to_string(),
            right_label: String::new(),
            song_id: None,
            album_id: None,
            playlist_id: None,
            artist_id: None,
            title: None,
            artist: None,
            album: None,
            cover_url: None,
            duration_ms: None,
            cover: CoverFetchState::default(),
        }
    }

    fn mixed_state() -> SearchState {
        SearchState {
            results: vec![
                item(SearchItemKind::Artist, "artist-1"),
                item(SearchItemKind::Artist, "artist-2"),
                item(SearchItemKind::Playlist, "playlist-1"),
                item(SearchItemKind::Song, "song-1"),
                item(SearchItemKind::Song, "song-2"),
            ],
            ..SearchState::default()
        }
    }

    /// 分区线的行数必须计入预算，否则最后一条会被挤出可见区。
    #[test]
    fn visible_end_accounts_for_divider_rows() {
        let state = SearchState {
            results: vec![
                item(SearchItemKind::Artist, "artist-1"),
                item(SearchItemKind::Playlist, "playlist-1"),
                item(SearchItemKind::Song, "song-1"),
            ],
            ..SearchState::default()
        };

        // 卡片 4 行 + 分区线 1 + 歌单 1 = 6：装不下第二段的分区线 + 单曲。
        assert_eq!(visible_search_end(&state, 0, 6, true), 2);
        // 关掉卡片后全部 1 行：3 条 + 2 条分区线 = 5 ≤ 6。
        assert_eq!(visible_search_end(&state, 0, 6, false), 3);
    }

    /// 极矮面板至少渲染一条，避免焦点落在看不见的条目上。
    #[test]
    fn visible_end_keeps_one_item_in_tiny_panel() {
        let state = mixed_state();
        assert_eq!(visible_search_end(&state, 0, 1, true), 1);
    }

    /// 序号按分区重新开始（同 kind 计数），跨窗口滚动时也不受影响。
    #[test]
    fn ordinal_restarts_per_section() {
        let state = mixed_state();
        assert_eq!(search_item_ordinal(&state, 1), 2);
        assert_eq!(search_item_ordinal(&state, 2), 1);
        assert_eq!(search_item_ordinal(&state, 4), 2);
    }
}
