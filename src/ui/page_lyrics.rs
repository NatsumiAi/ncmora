use crate::app::App;
use crate::data::config::Language;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub(crate) const PAGE_LYRICS_PANEL_HEIGHT: u16 = 4;

/// 歌词浮窗尺寸：宽 = 内容区 33%（夹在 18..=min(内容宽, 72)），高固定。
/// 内容区放不下时返回 `None`。
fn panel_size(content_area: Rect) -> Option<(u16, u16)> {
    if content_area.width < 18 || content_area.height < PAGE_LYRICS_PANEL_HEIGHT {
        return None;
    }

    let width = ((content_area.width as f32) * 0.33).round() as u16;
    let width = width.clamp(18, content_area.width.min(72));
    Some((width, PAGE_LYRICS_PANEL_HEIGHT))
}

fn clamp_pos(pos: (f32, f32)) -> (f32, f32) {
    (pos.0.clamp(0.0, 1.0), pos.1.clamp(0.0, 1.0))
}

/// 归一化位置（内容区内左上角，0..=1）→ 屏幕矩形。
fn panel_rect_at(content_area: Rect, size: (u16, u16), pos: (f32, f32)) -> Rect {
    let (width, height) = size;
    let (pos_x, pos_y) = clamp_pos(pos);
    let max_x = content_area.width.saturating_sub(width);
    let max_y = content_area.height.saturating_sub(height);

    Rect {
        x: content_area.x + (pos_x * max_x as f32).round() as u16,
        y: content_area.y + (pos_y * max_y as f32).round() as u16,
        width,
        height,
    }
}

/// 歌词浮窗矩形。`pos` 是归一化位置，默认 (1,1) = 右下角（历史行为）。
pub fn overlay_panel_area(content_area: Rect, pos: (f32, f32)) -> Rect {
    match panel_size(content_area) {
        Some(size) => panel_rect_at(content_area, size, pos),
        None => Rect::default(),
    }
}

/// 拖拽中的新位置：`grab` 是按下时鼠标相对浮窗左上角的偏移，
/// 这样拖动时浮窗不会跳到光标下。
pub fn pos_after_drag(
    content_area: Rect,
    panel: Rect,
    col: u16,
    row: u16,
    grab: (u16, u16),
) -> (f32, f32) {
    let max_x = content_area.width.saturating_sub(panel.width);
    let max_y = content_area.height.saturating_sub(panel.height);
    let x = col
        .saturating_sub(content_area.x)
        .saturating_sub(grab.0)
        .min(max_x);
    let y = row
        .saturating_sub(content_area.y)
        .saturating_sub(grab.1)
        .min(max_y);

    (
        if max_x == 0 {
            0.0
        } else {
            (x as f32 / max_x as f32).clamp(0.0, 1.0)
        },
        if max_y == 0 {
            0.0
        } else {
            (y as f32 / max_y as f32).clamp(0.0, 1.0)
        },
    )
}

/// 边缘吸附：浮窗中心落在哪个半区，就吸到对应的角。
pub fn snap_pos(content_area: Rect, panel: Rect) -> (f32, f32) {
    let panel_center_x = panel.x + panel.width / 2;
    let panel_center_y = panel.y + panel.height / 2;
    let area_center_x = content_area.x + content_area.width / 2;
    let area_center_y = content_area.y + content_area.height / 2;

    (
        if panel_center_x < area_center_x {
            0.0
        } else {
            1.0
        },
        if panel_center_y < area_center_y {
            0.0
        } else {
            1.0
        },
    )
}

/// 画歌词浮窗（按配置位置）并登记这一帧的几何：拖拽与点击拦截都以这份为准，
/// 免得各处各算一遍位置算错。
pub fn draw_page_lyrics_overlay(frame: &mut Frame, app: &mut App, content_area: Rect) {
    let panel = overlay_panel_area(content_area, app.page_lyrics_pos());
    app.set_page_lyrics_layout(content_area, panel);
    draw_page_lyrics_panel(frame, app, panel);
}

pub fn draw_page_lyrics_panel(frame: &mut Frame, app: &App, area: Rect) {
    if area.width < 8 || area.height < PAGE_LYRICS_PANEL_HEIGHT {
        return;
    }

    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.color_accent()))
            .style(base_bg_style(app)),
        area,
    );

    let inner = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.width == 0 || inner.height < 2 {
        return;
    }

    let (current, next) = app.current_page_lyric_lines();
    let (line1, line2) = if current.trim().is_empty() {
        match app.config.language {
            Language::Zh => ("暂无歌词".to_string(), String::new()),
            Language::En => ("No lyrics".to_string(), String::new()),
        }
    } else {
        (current, next)
    };

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            line1,
            Style::default()
                .fg(app.theme.color_accent2())
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[0],
    );

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            line2,
            Style::default().fg(app.theme.color_subtext()),
        )))
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[1],
    );
}

fn base_bg_style(app: &App) -> Style {
    if app.config.transparent_background {
        Style::default()
    } else {
        Style::default().bg(app.theme.color_base())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// 默认位置 (1,1) 必须与历史行为一致：贴内容区右下角。
    #[test]
    fn default_position_is_bottom_right() {
        let content = area(0, 0, 120, 30);
        let panel = overlay_panel_area(content, (1.0, 1.0));

        assert_eq!(panel.x + panel.width, content.x + content.width);
        assert_eq!(panel.y + panel.height, content.y + content.height);
        assert_eq!(panel.height, PAGE_LYRICS_PANEL_HEIGHT);
    }

    #[test]
    fn position_is_normalized_inside_the_content_area() {
        let content = area(3, 2, 100, 20);
        let size = panel_size(content).expect("放得下");

        for pos in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0)] {
            let panel = overlay_panel_area(content, pos);
            assert!(panel.x >= content.x && panel.y >= content.y);
            assert!(panel.x + panel.width <= content.x + content.width);
            assert!(panel.y + panel.height <= content.y + content.height);
            assert_eq!((panel.width, panel.height), size);
        }
    }

    /// 内容区放不下时返回空矩形（调用方据此跳过绘制与命中登记）。
    #[test]
    fn no_panel_when_the_area_is_too_small() {
        assert_eq!(
            overlay_panel_area(area(0, 0, 17, 30), (1.0, 1.0)),
            Rect::default()
        );
        assert_eq!(
            overlay_panel_area(area(0, 0, 80, 3), (1.0, 1.0)),
            Rect::default()
        );
    }

    /// 拖拽保持抓取偏移：光标下的那一点始终贴着浮窗同一处。
    #[test]
    fn drag_keeps_the_grab_offset() {
        let content = area(0, 0, 100, 20);
        let panel = overlay_panel_area(content, (1.0, 1.0));
        let grab = (5u16, 1u16);

        // 把光标放到内容区左上角 + grab：浮窗应回到 (0,0)
        let pos = pos_after_drag(content, panel, content.x + grab.0, content.y + grab.1, grab);
        assert_eq!(pos, (0.0, 0.0));

        // 越界拖动被钳制在内容区内
        let pos = pos_after_drag(content, panel, content.x + 500, content.y + 500, grab);
        assert_eq!(pos, (1.0, 1.0));
    }

    /// 吸附到最近的角。
    #[test]
    fn snap_picks_the_nearest_corner() {
        let content = area(0, 0, 100, 20);
        let size = panel_size(content).expect("放得下");

        let top_left = panel_rect_at(content, size, (0.0, 0.0));
        assert_eq!(snap_pos(content, top_left), (0.0, 0.0));

        let bottom_right = panel_rect_at(content, size, (1.0, 1.0));
        assert_eq!(snap_pos(content, bottom_right), (1.0, 1.0));

        // 稍稍越过后仍能吸回原来的角（半个浮窗的容差）
        let near_top_left = Rect {
            x: 1,
            y: 1,
            ..top_left
        };
        assert_eq!(snap_pos(content, near_top_left), (0.0, 0.0));
    }
}
