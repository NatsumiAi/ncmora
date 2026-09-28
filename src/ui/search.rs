use crate::app::{ARTIST_CARD_ROWS, App, SearchItemKind, SearchState};
use crate::data::config::Language;
use crate::ui::page_lyrics;
use crate::ui::player_bar;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

    let card = list_area.width >= ARTIST_CARD_MIN_WIDTH
        && usize::from(list_area.height) >= ARTIST_CARD_ROWS;
    app.search.set_viewport(usize::from(list_area.height), card);

    // 视口按行定位：条目高度不一，行滚动才能让顶部与底部同步移动。
    let top_row = app.search.effective_scroll_rows();
    let bottom_row = top_row.saturating_add(usize::from(list_area.height));

    for item_idx in 0..app.search.results.len() {
        let start_row = app.search.item_start_row(item_idx);
        if start_row >= bottom_row {
            break;
        }
        let end_row = app.search.item_end_row(item_idx);
        if end_row <= top_row {
            continue;
        }

        let kind = app.search.results[item_idx].kind;
        let divider = app.search.divider_rows(item_idx);
        let item_top_row = start_row + divider;

        // 分区线：单行，整行落在视口内才画（它不产生命中区）。
        if divider == 1 && start_row >= top_row {
            draw_search_divider(
                frame,
                app,
                Rect {
                    x: list_area.x,
                    y: list_area.y + (start_row - top_row) as u16,
                    width: list_area.width,
                    height: 1,
                },
            );
        }

        let visible_top = item_top_row.max(top_row);
        let visible_bottom = end_row.min(bottom_row);
        if visible_top >= visible_bottom {
            continue;
        }

        let focused = item_idx == app.search.focused_idx;
        let visible_rect = Rect {
            x: list_area.x,
            y: list_area.y + (visible_top - top_row) as u16,
            width: list_area.width,
            height: (visible_bottom - visible_top) as u16,
        };
        // 命中区覆盖条目可见部分：被裁切的卡片仍能点到露出来的那几行。
        app.push_search_item_hit(
            crate::app::HitRect {
                x: visible_rect.x,
                y: visible_rect.y,
                width: visible_rect.width,
                height: visible_rect.height,
            },
            item_idx,
        );

        if card && kind == SearchItemKind::Artist {
            render_clipped_artist_card(
                frame,
                app,
                list_area,
                top_row,
                item_top_row,
                item_idx,
                focused,
            );
        } else {
            let ordinal = search_item_ordinal(&app.search, item_idx);
            render_search_row(frame, app, visible_rect, item_idx, ordinal, focused);
        }
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

/// 按真实行位置画作者卡片，再把越出列表区（上/下边界）的那几行还原成页面底色：
/// 卡片因此可以被裁掉顶部若干行，滚多少行就裁多少行（ratatui 不支持按区域裁剪）。
fn render_clipped_artist_card(
    frame: &mut Frame,
    app: &mut App,
    list_area: Rect,
    top_row: usize,
    item_top_row: usize,
    item_idx: usize,
    focused: bool,
) {
    // 卡片在行空间里的真实位置可能落在列表区外，用有符号数算，便于做越界裁剪。
    let true_y = i32::from(list_area.y) + item_top_row as i32 - top_row as i32;
    if true_y < 0 {
        // 卡片顶端越出终端（极小窗口）：直接跳过，避免把内容画到错位的行上。
        return;
    }

    let card = Rect {
        x: list_area.x,
        y: true_y as u16,
        width: list_area.width,
        height: ARTIST_CARD_ROWS as u16,
    };
    render_artist_card(frame, app, card, item_idx, focused);

    let list_bottom = list_area.y.saturating_add(list_area.height);
    let card_bottom = card.y.saturating_add(card.height);
    let clear = |frame: &mut Frame, y: u16, height: u16| {
        if height == 0 {
            return;
        }
        let region = Rect {
            x: card.x,
            y,
            width: card.width,
            height,
        };
        frame.render_widget(Clear, region);
        frame.render_widget(Block::default().style(base_bg_style(app)), region);
    };

    if card.y < list_area.y {
        clear(frame, card.y, list_area.y - card.y);
    }
    if card_bottom > list_bottom {
        clear(frame, list_bottom, card_bottom - list_bottom);
    }
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
    use crate::app::SearchItem;

    fn item(kind: SearchItemKind, label: &str) -> SearchItem {
        SearchItem {
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

    /// 序号按分区重新开始（同 kind 计数），跨窗口滚动时也不受影响。
    #[test]
    fn ordinal_restarts_per_section() {
        let mut state = SearchState::default();
        state.set_results(
            vec![
                item(SearchItemKind::Artist, "artist-1"),
                item(SearchItemKind::Artist, "artist-2"),
                item(SearchItemKind::Playlist, "playlist-1"),
                item(SearchItemKind::Song, "song-1"),
                item(SearchItemKind::Song, "song-2"),
            ],
            0,
            false,
        );

        assert_eq!(search_item_ordinal(&state, 1), 2);
        assert_eq!(search_item_ordinal(&state, 2), 1);
        assert_eq!(search_item_ordinal(&state, 4), 2);
    }
}
