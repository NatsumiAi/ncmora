use std::time::Instant;

/// Search-box input and animation state.
pub(crate) struct InputController {
    pub search_box_input: String,
    pub search_box_cursor: usize,
    pub search_box_anim_height: u16,
    pub search_box_anim_started_at: Option<Instant>,
}

impl Default for InputController {
    fn default() -> Self {
        Self {
            search_box_input: String::new(),
            search_box_cursor: 0,
            search_box_anim_height: 0,
            search_box_anim_started_at: None,
        }
    }
}
