use super::{DownloadManager, DownloadRowCache, DownloadState, PlaylistPageKind};
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
