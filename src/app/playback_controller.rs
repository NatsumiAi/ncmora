use super::player::AudioPlayer;
use super::{LikeMachine, PlaybackRepeatMode, PlaybackRuntimeState, PlaybackTrack};

pub(crate) struct PlaybackController {
    pub audio_player: AudioPlayer,
    pub now_playing: Option<PlaybackTrack>,
    pub now_playing_liked: bool,
    pub(super) like_machine: LikeMachine,
    pub playback_queue: Vec<PlaybackTrack>,
    pub playback_queue_cover_url: Option<String>,
    pub playback_queue_source_id: Option<String>,
    pub playback_index: Option<usize>,
    pub playback_repeat_mode: PlaybackRepeatMode,
    pub playback_state: PlaybackRuntimeState,
}

impl PlaybackController {
    pub fn new(audio_player: AudioPlayer) -> Self {
        Self {
            audio_player,
            now_playing: None,
            now_playing_liked: false,
            like_machine: LikeMachine::default(),
            playback_queue: Vec::new(),
            playback_queue_cover_url: None,
            playback_queue_source_id: None,
            playback_index: None,
            playback_repeat_mode: PlaybackRepeatMode::Sequence,
            playback_state: PlaybackRuntimeState::Stopped,
        }
    }

    pub fn cycle_repeat_mode(&mut self) {
        self.playback_repeat_mode = self.playback_repeat_mode.next();
    }

    pub fn clear_like_state(&mut self) {
        self.now_playing_liked = false;
        self.like_machine.clear();
    }

    pub fn clear_queue(&mut self) {
        self.playback_queue.clear();
        self.playback_index = None;
        self.playback_state = PlaybackRuntimeState::Stopped;
        self.playback_repeat_mode = PlaybackRepeatMode::Sequence;
        self.playback_queue_cover_url = None;
        self.playback_queue_source_id = None;
    }

    pub fn replace_queue(
        &mut self,
        queue: Vec<PlaybackTrack>,
        index: Option<usize>,
        cover_url: Option<String>,
        source_id: Option<String>,
    ) {
        self.playback_queue = queue;
        self.playback_index = index;
        self.playback_queue_cover_url = cover_url;
        self.playback_queue_source_id = source_id;
    }
}
