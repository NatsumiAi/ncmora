use super::{DownloadPathEdit, HitRect, Overlay};
use std::time::Instant;

pub(crate) struct SettingsController {
    pub selected: usize,
    pub playback_selected: usize,
    pub lyrics_selected: usize,
    pub keybind_selected: usize,
    pub keybind_rebinding: Option<usize>,
    pub keybind_scroll: usize,
    pub download_selected: usize,
    pub download_path_edit: Option<DownloadPathEdit>,
    pub download_reset_armed: bool,
    pub item_hits: Vec<(HitRect, usize)>,
    pub last_click: Option<(Instant, Overlay, usize)>,
}

impl Default for SettingsController {
    fn default() -> Self {
        Self {
            selected: 0,
            playback_selected: 0,
            lyrics_selected: 0,
            keybind_selected: 0,
            keybind_rebinding: None,
            keybind_scroll: 0,
            download_selected: 0,
            download_path_edit: None,
            download_reset_armed: false,
            item_hits: Vec::new(),
            last_click: None,
        }
    }
}

impl SettingsController {
    pub fn reset_navigation(&mut self) {
        self.selected = 0;
        self.playback_selected = 0;
        self.lyrics_selected = 0;
        self.keybind_selected = 0;
        self.keybind_rebinding = None;
        self.keybind_scroll = 0;
        self.download_selected = 0;
        self.download_path_edit = None;
        self.download_reset_armed = false;
        self.last_click = None;
    }
}
