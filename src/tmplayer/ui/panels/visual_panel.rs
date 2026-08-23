use crate::tmplayer::app::state::{AppState, LyricLine};
use crate::tmplayer::data::config::VisualizeMode;
use crate::tmplayer::render::{oscilloscope_renderer, spectrum_renderer};
use crate::tmplayer::ui::borders::SOLID_BORDER;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(f: &mut Frame, lyric_area: Rect, spectrum_area: Rect, app: &mut AppState) {
    let outer = Rect {
        x: lyric_area.x,
        y: lyric_area.y,
        width: lyric_area.width,
        height: lyric_area.height.saturating_add(spectrum_area.height),
    };
    let outer_block = Block::default()
        .borders(Borders::ALL)
        .border_set(SOLID_BORDER)
        .style(Style::default().fg(app.theme.color_subtext()));
    f.render_widget(outer_block, outer);

    let inner = outer.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    if app.config.visualize == VisualizeMode::Off {
        render_full_lyrics(f, inner, app);
        return;
    }

    let lyric_h = lyric_area.height.saturating_sub(2).min(inner.height);
    let lyric_inner = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: lyric_h,
    };
    let spectrum_inner = Rect {
        x: inner.x,
        y: inner.y + lyric_h,
        width: inner.width,
        height: inner.height.saturating_sub(lyric_h),
    };

    let (l1, l2) = current_two_lines(app);
    if lyric_inner.height >= 1 && !l1.is_empty() {
        f.render_widget(
            Paragraph::new(current_line_widget(app, l1))
                .style(
                    Style::default()
                        .fg(app.theme.color_accent2())
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center),
            Rect {
                x: lyric_inner.x,
                y: lyric_inner.y,
                width: lyric_inner.width,
                height: 1,
            },
        );
    }
    if lyric_inner.height >= 2 && !l2.is_empty() {
        f.render_widget(
            Paragraph::new(l2)
                .style(Style::default().fg(app.theme.color_subtext()))
                .alignment(Alignment::Center),
            Rect {
                x: lyric_inner.x,
                y: lyric_inner.y + 1,
                width: lyric_inner.width,
                height: 1,
            },
        );
    }

    match app.config.visualize {
        VisualizeMode::Off => {}
        VisualizeMode::Bars => spectrum_renderer::render(f, spectrum_inner, app),
        VisualizeMode::Oscilloscope => oscilloscope_renderer::render(f, spectrum_inner, app),
    }
}

fn render_full_lyrics(f: &mut Frame, area: Rect, app: &AppState) {
    let lines = centered_lyric_window(app, area.height as usize);
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), area);
}

fn centered_lyric_window(app: &AppState, visible_rows: usize) -> Vec<Line<'static>> {
    let rows_count = visible_rows.max(1);
    let current_row = rows_count / 2;
    let mut rows = vec![Line::from(String::new()); rows_count];

    let Some(lines) = app.player.track.lyrics.as_ref() else {
        rows[current_row] = Line::from(Span::styled(
            no_lyrics_label(app),
            Style::default().fg(app.theme.color_subtext()),
        ));
        return rows;
    };

    if lines.is_empty() {
        rows[current_row] = Line::from(Span::styled(
            no_lyrics_label(app),
            Style::default().fg(app.theme.color_subtext()),
        ));
        return rows;
    }

    let pos_ms = app.player.position.as_millis() as u64;
    let current_idx = current_lyric_index(lines, pos_ms);
    let has_translations = lines.iter().any(|line| {
        line.translation
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
    });
    let line_height = if has_translations { 2 } else { 1 };
    let visible_lines = (rows_count / line_height).max(1);
    let current_row = visible_lines / 2;

    for line_row in 0..visible_lines {
        let lyric_idx = current_idx as isize + line_row as isize - current_row as isize;
        if lyric_idx < 0 || lyric_idx >= lines.len() as isize {
            continue;
        }

        let lyric = &lines[lyric_idx as usize];
        let style = if lyric_idx as usize == current_idx {
            Style::default()
                .fg(app.theme.color_accent2())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.color_subtext())
        };
        let row = line_row * line_height;
        rows[row] = if lyric_idx as usize == current_idx && !lyric.words.is_empty() {
            timed_line(lyric, pos_ms, app)
        } else {
            Line::from(Span::styled(lyric.text.clone(), style))
        };
        if has_translations {
            if let Some(translation) = lyric
                .translation
                .as_deref()
                .filter(|text| !text.trim().is_empty())
            {
                if row + 1 < rows.len() {
                    rows[row + 1] = Line::from(Span::styled(
                        translation.to_string(),
                        Style::default()
                            .fg(app.theme.color_subtext())
                            .add_modifier(Modifier::ITALIC),
                    ));
                }
            }
        }
    }

    rows
}

fn no_lyrics_label(app: &AppState) -> &'static str {
    match app.language {
        crate::data::config::Language::Zh => "暂无歌词",
        crate::data::config::Language::En => "No lyrics",
    }
}

fn current_two_lines(app: &AppState) -> (&str, &str) {
    let Some(lines) = app.player.track.lyrics.as_ref() else {
        return ("", "");
    };
    if lines.is_empty() {
        return ("", "");
    }

    let pos_ms = app.player.position.as_millis() as u64;
    let idx = current_lyric_index(lines, pos_ms);

    let l1 = lines.get(idx).map(|l| l.text.as_str()).unwrap_or("");
    let l2 = lines
        .get(idx)
        .and_then(|line| line.translation.as_deref())
        .filter(|text| !text.trim().is_empty())
        .or_else(|| lines.get(idx + 1).map(|l| l.text.as_str()))
        .unwrap_or("");
    (l1, l2)
}

fn current_line_widget(app: &AppState, text: &str) -> Line<'static> {
    let Some(lines) = app.player.track.lyrics.as_ref() else {
        return Line::from(text.to_string());
    };
    let idx = current_lyric_index(lines, app.player.position.as_millis() as u64);
    let Some(line) = lines.get(idx) else {
        return Line::from(text.to_string());
    };
    if line.words.is_empty() {
        Line::from(text.to_string())
    } else {
        timed_line(line, app.player.position.as_millis() as u64, app)
    }
}

fn timed_line(line: &LyricLine, pos_ms: u64, app: &AppState) -> Line<'static> {
    let mut rendered = Line::default();
    for word in &line.words {
        let style = if pos_ms >= word.end_ms {
            Style::default().fg(app.theme.color_accent2())
        } else if pos_ms >= word.start_ms {
            Style::default()
                .fg(app.theme.color_accent2())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.color_subtext())
        };
        rendered.push_span(Span::styled(word.text.clone(), style));
    }
    rendered
}

fn current_lyric_index(lines: &[LyricLine], pos_ms: u64) -> usize {
    let mut idx = 0;
    for (i, line) in lines.iter().enumerate() {
        if line.start_ms <= pos_ms {
            idx = i;
        } else {
            break;
        }
    }
    idx
}
