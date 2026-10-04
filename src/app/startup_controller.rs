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

    pub fn begin(&mut self, target: Page) {
        self.progress = 0.0;
        self.started_at = Some(Instant::now());
        self.complete_started_at = None;
        self.complete_requested = false;
        self.target = target;
    }

    pub fn finish(&mut self) {
        self.complete_requested = true;
        self.complete_started_at.get_or_insert_with(Instant::now);
    }

    pub fn current_progress(&self) -> f32 {
        super::startup_loading_progress(
            self.init.step_done(),
            self.init.step_total(),
            self.init.step_elapsed(),
            self.complete_started_at.map(|time| time.elapsed().as_secs_f32()),
            self.complete_requested,
        )
    }

    pub fn tick(&mut self) -> Option<Page> {
        let Some(started_at) = self.started_at else {
            self.started_at = Some(Instant::now());
            return None;
        };
        self.progress = self.current_progress();
        let ramp_done = self.complete_started_at
            .is_some_and(|time| time.elapsed().as_secs_f32() >= super::STARTUP_LOADING_COMPLETE_RAMP_SECS);
        if self.complete_requested && ramp_done
            && started_at.elapsed().as_secs_f32() >= super::STARTUP_LOADING_MIN_VISIBLE_SECS
        {
            self.progress = 0.0;
            self.started_at = None;
            self.complete_started_at = None;
            self.complete_requested = false;
            Some(self.target)
        } else {
            None
        }
    }
}
