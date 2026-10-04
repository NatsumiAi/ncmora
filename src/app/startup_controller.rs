use super::{Page, StartupInit};
use std::time::Instant;

/// Startup task and loading-screen progress state.
pub(crate) struct StartupController {
    pub init: StartupInit,
    pub progress: f32,
    pub target: Page,
    pub started_at: Option<Instant>,
    pub complete_started_at: Option<Instant>,
    pub complete_requested: bool,
}

impl StartupController {
    pub fn detached() -> Self {
        Self {
            init: StartupInit::detached(),
            progress: 0.0,
            target: Page::Login,
            started_at: None,
            complete_started_at: None,
            complete_requested: false,
        }
    }
}
