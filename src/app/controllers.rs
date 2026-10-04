use super::{SearchItem, SearchItemKind, SearchScope, next_list_generation};
use std::ops::Range;

/// 搜索页控制器：持有结果、焦点、分页游标和渲染布局索引。
pub(crate) struct SearchController {
    pub query: String,
    pub focused_idx: usize,
    results: Vec<SearchItem>,
    pub status_line: String,
    pub scope: SearchScope,
    pub next_offset: usize,
    pub has_more: bool,
    pub scroll_rows: usize,
    view_rows: usize,
    card_mode: bool,
    compact_layout_index: Vec<usize>,
    card_layout_index: Vec<usize>,
    compact_total_rows: usize,
    card_total_rows: usize,
    item_ordinals: Vec<usize>,
    generation: u64,
}

impl Default for SearchController {
    fn default() -> Self {
        Self {
            query: String::new(),
            focused_idx: 0,
            results: Vec::new(),
            status_line: "输入关键词后按 Enter 搜索".to_string(),
            scope: SearchScope::Mixed,
            next_offset: 0,
            has_more: false,
            scroll_rows: 0,
            view_rows: 1,
            card_mode: false,
            compact_layout_index: Vec::new(),
            card_layout_index: Vec::new(),
            compact_total_rows: 0,
            card_total_rows: 0,
            item_ordinals: Vec::new(),
            generation: next_list_generation(),
        }
    }
}

impl SearchController {
    fn rebuild_layout_index(&mut self) {
        self.compact_layout_index.clear();
        self.card_layout_index.clear();
        self.item_ordinals.clear();
        self.compact_layout_index.reserve(self.results.len());
        self.card_layout_index.reserve(self.results.len());
        self.item_ordinals.reserve(self.results.len());

        let mut compact_row = 0usize;
        let mut card_row = 0usize;
        let mut counts = [0usize; 4];
        for index in 0..self.results.len() {
            let kind = self.results[index].kind;
            let divider = usize::from(index > 0 && self.results[index - 1].kind != kind);
            self.compact_layout_index.push(compact_row);
            self.card_layout_index.push(card_row);
            let count = match kind {
                SearchItemKind::Song => &mut counts[0],
                SearchItemKind::Album => &mut counts[1],
                SearchItemKind::Artist => &mut counts[2],
                SearchItemKind::Playlist => &mut counts[3],
            };
            *count += 1;
            self.item_ordinals.push(*count);
            compact_row = compact_row.saturating_add(divider + kind.rows(false));
            card_row = card_row.saturating_add(divider + kind.rows(true));
        }
        self.compact_total_rows = compact_row;
        self.card_total_rows = card_row;
    }

    fn layout_index(&self) -> &[usize] {
        if self.card_mode {
            &self.card_layout_index
        } else {
            &self.compact_layout_index
        }
    }

    fn row_span(&self, index: usize) -> usize {
        let kind = self.results[index].kind;
        let divider = usize::from(index > 0 && self.results[index - 1].kind != kind);
        divider + kind.rows(self.card_mode)
    }

    pub fn item_start_row(&self, index: usize) -> usize {
        self.layout_index().get(index).copied().unwrap_or(self.total_rows())
    }

    pub fn item_end_row(&self, index: usize) -> usize {
        self.item_start_row(index) + self.row_span(index)
    }

    pub fn item_ordinal(&self, index: usize) -> usize {
        self.item_ordinals.get(index).copied().unwrap_or(1)
    }

    pub fn divider_rows(&self, index: usize) -> usize {
        usize::from(index > 0 && self.results[index - 1].kind != self.results[index].kind)
    }

    fn total_rows(&self) -> usize {
        if self.card_mode { self.card_total_rows } else { self.compact_total_rows }
    }

    fn max_scroll_rows(&self) -> usize {
        self.total_rows().saturating_sub(self.view_rows.max(1))
    }

    fn clamp_scroll(&mut self) {
        self.scroll_rows = self.scroll_rows.min(self.max_scroll_rows());
    }

    fn ensure_focus_visible(&mut self) {
        if self.results.is_empty() {
            self.focused_idx = 0;
            self.scroll_rows = 0;
            return;
        }
        self.focused_idx = self.focused_idx.min(self.results.len() - 1);
        let view = self.view_rows.max(1);
        let start = self.item_start_row(self.focused_idx);
        let end = self.item_end_row(self.focused_idx);
        if end > self.scroll_rows.saturating_add(view) { self.scroll_rows = end - view; }
        if start < self.scroll_rows { self.scroll_rows = start; }
        self.clamp_scroll();
    }

    pub fn set_viewport(&mut self, view_rows: usize, card_mode: bool) {
        self.view_rows = view_rows.max(1);
        self.card_mode = card_mode;
        if self.compact_layout_index.len() != self.results.len()
            || self.card_layout_index.len() != self.results.len()
        { self.rebuild_layout_index(); }
        self.ensure_focus_visible();
    }

    pub fn effective_scroll_rows(&self) -> usize { self.scroll_rows.min(self.max_scroll_rows()) }

    /// Returns only entries intersecting the row viewport, using cached starts.
    pub fn visible_range(&self, top: usize, bottom: usize) -> Range<usize> {
        let starts = self.layout_index();
        let mut first = 0usize;
        let mut high = starts.len();
        while first < high {
            let mid = first + (high - first) / 2;
            if self.item_end_row(mid) <= top { first = mid + 1; } else { high = mid; }
        }
        let mut last = first;
        high = starts.len();
        while last < high {
            let mid = last + (high - last) / 2;
            if starts[mid] < bottom { last = mid + 1; } else { high = mid; }
        }
        first..last
    }

    pub fn page_items(&self) -> usize {
        let top = self.effective_scroll_rows();
        let bottom = top.saturating_add(self.view_rows.max(1));
        self.visible_range(top, bottom).len().max(1)
    }

    pub fn set_focus(&mut self, index: usize) {
        if self.results.is_empty() { self.focused_idx = 0; self.scroll_rows = 0; return; }
        self.focused_idx = index.min(self.results.len() - 1);
        self.ensure_focus_visible();
    }

    pub fn focus_next(&mut self) -> bool {
        if self.results.is_empty() || self.focused_idx + 1 >= self.results.len() { return false; }
        self.focused_idx += 1;
        self.ensure_focus_visible();
        true
    }

    pub fn focus_prev(&mut self) -> bool {
        if self.results.is_empty() || self.focused_idx == 0 { return false; }
        self.focused_idx -= 1;
        self.ensure_focus_visible();
        true
    }

    pub fn set_results(&mut self, results: Vec<SearchItem>, next_offset: usize, has_more: bool) {
        self.results = results;
        self.focused_idx = 0;
        self.next_offset = next_offset;
        self.has_more = has_more;
        self.scroll_rows = 0;
        self.rebuild_layout_index();
        self.generation = next_list_generation();
        self.ensure_focus_visible();
    }

    pub fn generation(&self) -> u64 { self.generation }

    pub fn append_results(&mut self, mut results: Vec<SearchItem>) -> usize {
        let added = results.len();
        self.results.append(&mut results);
        self.rebuild_layout_index();
        self.generation = next_list_generation();
        self.clamp_scroll();
        added
    }
    pub fn results(&self) -> &[SearchItem] {
        &self.results
    }

    pub fn len(&self) -> usize {
        self.results.len()
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{CoverFetchState, SearchItem};

    fn item(kind: SearchItemKind, label: &str) -> SearchItem {
        SearchItem { kind, left_label: label.to_string(), right_label: String::new(), song_id: None, album_id: None, playlist_id: None, artist_id: None, title: None, artist: None, album: None, cover_url: None, duration_ms: None, cover: CoverFetchState::default() }
    }

    #[test]
    fn mixed_sections_cache_rows_and_ordinals() {
        let mut state = SearchController::default();
        state.set_results(vec![item(SearchItemKind::Artist, "a1"), item(SearchItemKind::Artist, "a2"), item(SearchItemKind::Song, "s1"), item(SearchItemKind::Playlist, "p1")], 0, false);
        state.set_viewport(20, true);
        assert_eq!(state.item_start_row(0), 0);
        assert_eq!(state.item_start_row(1), 4);
        assert_eq!(state.item_start_row(2), 8);
        assert_eq!(state.item_start_row(3), 10);
        assert_eq!(state.item_ordinal(2), 1);
        assert_eq!(state.item_ordinal(3), 1);
    }

    #[test]
    fn append_rebuilds_both_layouts_and_generation() {
        let mut state = SearchController::default();
        state.set_results(vec![item(SearchItemKind::Artist, "a")], 1, true);
        state.set_viewport(8, true);
        let generation = state.generation();
        assert_eq!(state.append_results(vec![item(SearchItemKind::Song, "s")]), 1);
        assert_ne!(state.generation(), generation);
        assert_eq!(state.item_start_row(1), 5);
        state.set_viewport(8, false);
        assert_eq!(state.item_start_row(1), 2);
    }

    #[test]
    fn resize_and_focus_keep_visible_range_deterministic() {
        let mut state = SearchController::default();
        state.set_results((0..8).map(|i| item(SearchItemKind::Song, &format!("s{i}"))).collect(), 0, false);
        state.set_viewport(3, false);
        state.set_focus(6);
        assert_eq!(state.effective_scroll_rows(), 4);
        assert_eq!(state.visible_range(4, 7), 4..7);
        state.set_viewport(2, false);
        assert_eq!(state.effective_scroll_rows(), 5);
    }
}
