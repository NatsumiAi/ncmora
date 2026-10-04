use crate::data::icons::UiIcons;
use crate::tmplayer::app::state::{AppState, PlaybackState, RepeatMode};
use crate::tmplayer::utils::input::Action;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

fn glyphs(app: &AppState) -> [&'static str; 4] {
    let icons = UiIcons::for_mode(app.config.icon_mode);
    let repeat = match app.player.repeat_mode {
        RepeatMode::Sequence => icons.sequence(),
        RepeatMode::Shuffle => icons.shuffle(),
        RepeatMode::LoopAll => icons.loop_all(),
        RepeatMode::LoopOne => icons.loop_one(),
    };
    [
        icons.previous(),
        icons.play_pause(app.player.playback == PlaybackState::Playing),
        icons.next(),
        repeat,
    ]
}

pub fn render(f: &mut Frame, area: Rect, app: &AppState) {
    let [previous, play, next, repeat] = glyphs(app);
    let text = Style::default().fg(app.theme.color_text());
    let line = Line::from(vec![
        Span::styled(previous, text),
        Span::styled(" ", text),
        Span::styled(play, text),
        Span::styled(" ", text),
        Span::styled(next, text),
        Span::styled(" ", text),
        Span::styled(repeat, Style::default().fg(app.theme.color_subtext())),
    ]);

    f.render_widget(
        Paragraph::new(line)
            .style(Style::default())
            .alignment(ratatui::layout::Alignment::Center),
        area,
    );
}

pub fn hit_test(area: Rect, app: &AppState, col: u16, row: u16) -> Option<Action> {
    if !area.contains((col, row).into()) {
        return None;
    }
    let [s_prev, s_play, s_next, repeat_symbol] = glyphs(app);
    let text_w = [s_prev, s_play, s_next, repeat_symbol]
        .into_iter()
        .map(UnicodeWidthStr::width)
        .sum::<usize>() as u16
        + 3;

    // Paragraph centers the already-truncated line, rounding each half separately.
    let start_x = area.x + (area.width / 2).saturating_sub(text_w.min(area.width) / 2);
    if col < start_x || col >= start_x + text_w {
        return None;
    }
    let mut x = start_x;

    let w_prev = UnicodeWidthStr::width(s_prev) as u16;
    if col >= x && col < x + w_prev {
        return Some(Action::Prev);
    }
    x += w_prev;
    x += 1; // sep

    let w_play = UnicodeWidthStr::width(s_play) as u16;
    if col >= x && col < x + w_play {
        return Some(Action::TogglePlayPause);
    }
    x += w_play;
    x += 1; // sep

    let w_next = UnicodeWidthStr::width(s_next) as u16;
    if col >= x && col < x + w_next {
        return Some(Action::Next);
    }
    x += w_next;
    x += 1; // sep

    let w_mode = UnicodeWidthStr::width(repeat_symbol) as u16;
    if col >= x && col < x + w_mode {
        return Some(Action::ToggleRepeatMode);
    }

    None
}
