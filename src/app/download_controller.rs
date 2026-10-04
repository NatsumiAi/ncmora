use super::download::DownloadEvent;
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
    pub page_kind: PlaylistPageKind,
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
