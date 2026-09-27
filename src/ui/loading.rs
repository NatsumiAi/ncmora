use crate::app::App;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

const DEFAULT_OPENING_TITLE: &str = " ██████╗███╗   ██╗███╗   ███╗██████╗ ██╗      █████╗ ██╗   ██╗███████╗██████╗ \n██╔════╝████╗  ██║████╗ ████║██╔══██╗██║     ██╔══██╗╚██╗ ██╔╝██╔════╝██╔══██╗\n██║     ██╔██╗ ██║██╔████╔██║██████╔╝██║     ███████║ ╚████╔╝ █████╗  ██████╔╝\n██║     ██║╚██╗██║██║╚██╔╝██║██╔═══╝ ██║     ██╔══██║  ╚██╔╝  ██╔══╝  ██╔══██╗\n╚██████╗██║ ╚████║██║ ╚═╝ ██║██║     ███████╗██║  ██║   ██║   ███████╗██║  ██║\n ╚═════╝╚═╝  ╚═══╝╚═╝     ╚═╝╚═╝     ╚══════╝╚═╝  ╚═╝   ╚═╝   ╚══════╝╚═╝  ╚═╝";

pub fn draw_loading(frame: &mut Frame, app: &App) {
    let size = frame.area();

    frame.render_widget(Block::default().style(base_bg_style(app)), size);

    if size.width < 36 || size.height < 10 {
        // 面积不够放下标题时只画一根进度条：加载页不出现任何文字。
        let bar_w = size.width.saturating_sub(2);
        if bar_w >= 4 {
            draw_progress_bar(
                frame,
                app,
                Rect {
                    x: size.x.saturating_add(1),
                    y: size.y + size.height / 2,
                    width: bar_w,
                    height: 1,
                },
            );
        }
        return;
    }

    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
        .split(size);

    let title_block = opening_title_block(&app.config.default_opening_title);
    let title_lines = title_block.lines().count().max(1) as u16;
    let title_area = centered_rect(areas[0].width, title_lines.min(areas[0].height), areas[0]);

    frame.render_widget(
        Paragraph::new(title_block)
            .style(
                Style::default()
                    .fg(app.theme.color_accent())
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center),
        title_area,
    );

    let bar_w = areas[1].width.saturating_sub(4);
    if bar_w == 0 {
        return;
    }
    let bar_y = areas[1].y + areas[1].height.saturating_sub(1) / 2;
    let bar_area = Rect {
        x: areas[1].x.saturating_add(2),
        y: bar_y,
        width: bar_w,
        height: 1,
    };

    draw_progress_bar(frame, app, bar_area);
}

/// 进度条本体：只有方块，没有任何文字。小窗口与常规布局共用。
fn draw_progress_bar(frame: &mut Frame, app: &App, area: Rect) {
    let progress = app
        .startup_loading_progress_for_width(area.width)
        .clamp(0.0, 1.0);
    let filled = ((area.width as f32) * progress).round() as u16;
    let filled = filled.min(area.width);

    let filled_usize = filled as usize;
    let mut spans = Vec::with_capacity(filled_usize.saturating_add(1));
    for idx in 0..filled_usize {
        let t = if filled_usize <= 1 {
            1.0
        } else {
            idx as f32 / (filled_usize - 1) as f32
        };
        spans.push(Span::styled(
            "█",
            Style::default().fg(mix_color(
                app.theme.color_accent3(),
                app.theme.color_accent2(),
                t,
            )),
        ));
    }
    let remain = area.width.saturating_sub(filled) as usize;
    if remain > 0 {
        spans.push(Span::styled(
            "░".repeat(remain),
            Style::default().fg(app.theme.color_surface()),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn opening_title_block(custom: &str) -> String {
    if custom.trim().is_empty() {
        return DEFAULT_OPENING_TITLE.to_string();
    }

    custom.replace("\\n", "\n")
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn base_bg_style(app: &App) -> Style {
    if app.config.transparent_background {
        Style::default()
    } else {
        Style::default().bg(app.theme.color_base())
    }
}

fn mix_color(start: Color, end: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (start, end) {
        (Color::Rgb(sr, sg, sb), Color::Rgb(er, eg, eb)) => {
            let r = (sr as f32 + (er as f32 - sr as f32) * t) as u8;
            let g = (sg as f32 + (eg as f32 - sg as f32) * t) as u8;
            let b = (sb as f32 + (eb as f32 - sb as f32) * t) as u8;
            Color::Rgb(r, g, b)
        }
        _ => end,
    }
}
