#[derive(Debug, Clone)]
pub struct PlaylistItem {
    pub song_id: Option<String>,
    pub title: String,
}

#[derive(Debug, Default, Clone)]
pub struct Playlist {
    pub items: Vec<PlaylistItem>,
    pub selected: usize,
    pub current: Option<usize>,
}

impl Playlist {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn clamp_selected(&mut self) {
        if self.items.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.items.len() {
            self.selected = self.items.len() - 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1).min(self.items.len() - 1);
        }
    }
}
