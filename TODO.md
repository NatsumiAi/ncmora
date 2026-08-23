# NCMora TODO

## Completed

- [x] Persist the current playback position with the playback queue.
- [x] Restore the saved position when `resume_last_position` is enabled.
- [x] Expose resume-position behavior in Playback Settings.
- [x] Remove the obsolete `kitty_graphics` template option.

## Next

- [ ] Restore adaptive high-quality image protocols (Kitty/Sixel where supported), with a Halfblocks fallback.
- [x] Replace the external cava dependency with an in-process playback sample analyzer.
- [x] Add an explicit loading/error placeholder for unavailable home covers.
- [ ] Add a configurable dim layer for transparent backgrounds to keep text readable over busy terminal backgrounds.
- [ ] Reconnect the original local-audio metadata pipeline (lyrics, cover art, and optional AcoustID fingerprinting), or remove the unused local-audio settings.
- [ ] Decide whether to restore a dedicated album page or document the playlist-layout behavior as intentional.
- [x] Decide whether release artifacts should bundle `cava` instead of requiring an external executable: no bundle is needed.
- [ ] Add integration coverage for login/session restore, search suffixes, playback/seek, fullscreen sync, and restart recovery.
