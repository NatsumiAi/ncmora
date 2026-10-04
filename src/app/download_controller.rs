use super::download::{DownloadEvent, DownloadRequest};
use super::{DownloadManager, DownloadRowCache, DownloadState, PlaylistPageKind};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

/// Download UI/cache state. Worker ownership remains in DownloadManager.
pub(crate) struct DownloadController {
    pub manager: DownloadManager,
    pub playlist_cache: DownloadRowCache,
    pub search_cache: DownloadRowCache,
    pub rows_epoch: u64,
    pub spinner_start: Instant,
    pub now_playing_state: DownloadState,
    pub root: Option<PathBuf>,
    pub(super) page_kind: PlaylistPageKind,
    pub(super) pending_intents: PendingDownloads,
}
impl DownloadController {
    pub fn poll(&mut self) -> Vec<DownloadEvent> {
        self.manager.poll()
    }

    pub fn refresh_playlist(
        &mut self,
        epoch: (u64, u64),
        tracks: &[super::PlaylistTrack],
        root: Option<&std::path::Path>,
    ) {
        self.playlist_cache.refresh(epoch, &mut self.manager, || {
            super::playlist_download_rows(tracks, root)
        });
    }

    pub fn refresh_search(
        &mut self,
        epoch: (u64, u64),
        results: &[super::SearchItem],
        root: Option<&std::path::Path>,
    ) {
        self.search_cache.refresh(epoch, &mut self.manager, || {
            super::search_download_rows(results, root)
        });
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct DownloadOrigin {
    root: u64,
    session: u64,
}

struct PendingDownload {
    origin: DownloadOrigin,
    request: DownloadRequest,
}

/// User clicks waiting for disk status retain their order and original target.
/// Only explicit toggles cancel them; polling never replays a toggle.
#[derive(Default)]
pub(super) struct PendingDownloads {
    origin: DownloadOrigin,
    queue: VecDeque<PendingDownload>,
}

impl PendingDownloads {
    pub fn origin(&self) -> DownloadOrigin {
        self.origin
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn cancel(&mut self, song_id: &str) -> bool {
        let Some(index) = self
            .queue
            .iter()
            .position(|pending| pending.request.song_id == song_id)
        else {
            return false;
        };
        self.queue.remove(index);
        true
    }

    pub fn defer(&mut self, origin: DownloadOrigin, request: DownloadRequest) {
        if origin == self.origin {
            self.queue.push_back(PendingDownload { origin, request });
        }
    }

    pub fn invalidate_root(&mut self) {
        self.origin.root = self.origin.root.wrapping_add(1);
        self.queue.clear();
    }

    pub fn invalidate_session(&mut self) {
        self.origin.session = self.origin.session.wrapping_add(1);
        self.queue.clear();
    }

    /// Unknown files block later clicks; existing/busy songs are discarded, never replaced.
    pub fn take_ready(
        &mut self,
        mut state: impl FnMut(&DownloadRequest) -> Option<DownloadState>,
    ) -> Option<DownloadRequest> {
        loop {
            let pending = self.queue.front()?;
            if pending.origin != self.origin {
                self.queue.pop_front();
                continue;
            }
            match state(&pending.request) {
                None => return None,
                Some(DownloadState::NotDownloaded) => {
                    return self.queue.pop_front().map(|pending| pending.request);
                }
                Some(DownloadState::Done | DownloadState::Downloading) => {
                    self.queue.pop_front();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::download::DownloadTarget;
    use crate::data::config::AudioQuality;

    fn request(song_id: &str) -> DownloadRequest {
        DownloadRequest {
            song_id: song_id.to_string(),
            level: AudioQuality::Exhigh,
            title: song_id.to_string(),
            artist: "artist".to_string(),
            album: "album".to_string(),
            target: DownloadTarget {
                dir: PathBuf::from("downloads"),
                base: song_id.to_string(),
            },
        }
    }

    #[test]
    fn second_toggle_cancels_pending_without_admission() {
        let mut pending = PendingDownloads::default();
        pending.defer(pending.origin(), request("first"));
        assert!(pending.take_ready(|_| None).is_none());
        assert!(pending.cancel("first"));
        assert!(
            pending
                .take_ready(|_| panic!("cancelled intent must not look up a file"))
                .is_none()
        );
        assert!(!pending.cancel("first"));
    }

    #[test]
    fn single_click_is_admitted_once_when_absence_becomes_known() {
        let mut pending = PendingDownloads::default();
        pending.defer(pending.origin(), request("first"));
        assert!(pending.take_ready(|_| None).is_none());
        assert!(pending.take_ready(|_| None).is_none());
        let admitted = pending
            .take_ready(|_| Some(DownloadState::NotDownloaded))
            .unwrap();
        assert_eq!(admitted.song_id, "first");
        assert_eq!(admitted.target.dir, PathBuf::from("downloads"));
        assert!(
            pending
                .take_ready(|_| panic!("intent must be consumed once"))
                .is_none()
        );
    }

    #[test]
    fn different_songs_keep_click_order_and_can_be_cancelled_independently() {
        let mut pending = PendingDownloads::default();
        let origin = pending.origin();
        pending.defer(origin, request("first"));
        pending.defer(origin, request("cancelled"));
        pending.defer(origin, request("third"));
        assert!(pending.cancel("cancelled"));
        assert!(
            pending
                .take_ready(|request| {
                    assert_eq!(request.song_id, "first");
                    None
                })
                .is_none()
        );
        assert_eq!(
            pending
                .take_ready(|_| Some(DownloadState::NotDownloaded))
                .unwrap()
                .song_id,
            "first"
        );
        assert_eq!(
            pending
                .take_ready(|_| Some(DownloadState::NotDownloaded))
                .unwrap()
                .song_id,
            "third"
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn existing_or_busy_songs_do_not_produce_replacement_requests() {
        let mut pending = PendingDownloads::default();
        let origin = pending.origin();
        pending.defer(origin, request("existing"));
        pending.defer(origin, request("busy"));
        pending.defer(origin, request("absent"));
        let admitted = pending
            .take_ready(|request| match request.song_id.as_str() {
                "existing" => Some(DownloadState::Done),
                "busy" => Some(DownloadState::Downloading),
                "absent" => Some(DownloadState::NotDownloaded),
                _ => panic!("unexpected request"),
            })
            .unwrap();
        assert_eq!(admitted.song_id, "absent");
        assert!(pending.is_empty());
    }

    #[test]
    fn root_change_clears_waiters_and_rejects_old_root_origin() {
        let mut pending = PendingDownloads::default();
        let old_origin = pending.origin();
        pending.defer(old_origin, request("waiting"));
        pending.invalidate_root();
        pending.defer(old_origin, request("late_old_root"));
        assert!(
            pending
                .take_ready(|_| panic!("stale roots must not query or admit"))
                .is_none()
        );
        pending.defer(pending.origin(), request("new_root"));
        assert_eq!(
            pending
                .take_ready(|_| Some(DownloadState::NotDownloaded))
                .unwrap()
                .song_id,
            "new_root"
        );
    }

    #[test]
    fn logout_clears_waiters_and_rejects_prelogout_origin() {
        let mut pending = PendingDownloads::default();
        let old_origin = pending.origin();
        pending.defer(old_origin, request("waiting"));
        pending.invalidate_session();
        pending.defer(old_origin, request("late_old_session"));
        assert!(
            pending
                .take_ready(|_| panic!("old sessions must not query or admit"))
                .is_none()
        );
        pending.defer(pending.origin(), request("new_session"));
        assert_eq!(
            pending
                .take_ready(|_| Some(DownloadState::NotDownloaded))
                .unwrap()
                .song_id,
            "new_session"
        );
    }
}
