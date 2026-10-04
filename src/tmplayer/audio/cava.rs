use crate::state;
use anyhow::{Context as _, Result};
use compio::time::{Interval, interval};
use futures::stream::unfold;
use futures::{Stream, StreamExt};
use see::unsync::Receiver;
use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver as StdReceiver, SyncSender, TrySendError};
use std::sync::{Arc, LazyLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_BARS: usize = 96;
pub const MINI_BARS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CavaChannels {
    Stereo,
    Mono,
}

#[derive(Debug, Clone, Copy)]
pub struct CavaSnapshot {
    bars: usize,
    left: [f32; MAX_BARS],
    right: [f32; MAX_BARS],
}

impl Default for CavaSnapshot {
    fn default() -> Self {
        Self { bars: 0, left: [0.0; MAX_BARS], right: [0.0; MAX_BARS] }
    }
}

impl CavaSnapshot {
    pub fn bars(&self) -> usize { self.bars }
    pub fn left(&self) -> &[f32] { &self.left[..self.bars] }
    pub fn right(&self) -> &[f32] { &self.right[..self.bars] }

    pub fn copy_stereo_into(&self, left: &mut [f32], right: &mut [f32]) -> usize {
        let n = self.bars.min(left.len()).min(right.len());
        left[..n].copy_from_slice(&self.left[..n]);
        right[..n].copy_from_slice(&self.right[..n]);
        n
    }

    pub fn mono_into(&self, out: &mut [f32]) -> usize {
        let n = self.bars.min(out.len());
        for (dst, (left, right)) in out[..n]
            .iter_mut()
            .zip(self.left[..n].iter().zip(self.right[..n].iter()))
        {
            *dst = (*left + *right) * 0.5;
        }
        n
    }

    pub fn mini_mono(&self) -> [f32; MINI_BARS] {
        let mut out = [0.0; MINI_BARS];
        let _ = self.mono_into(&mut out);
        out
    }
}

pub struct MiniCavaState {
    pub event: Receiver<[f32; MINI_BARS]>,
    service: CavaService,
}

fn interval_stream(interval: Interval) -> impl Stream<Item = Instant> {
    unfold(interval, async |mut interval| Some((interval.tick().await, interval)))
}

impl MiniCavaState {
    pub fn try_new(cfg: CavaConfig) -> Result<Self> {
        let freq = cfg.framerate_hz.clamp(1, 120);
        let service = CavaService::new();
        service.set_desired(Some(cfg));
        let poll_period = Duration::from_millis((1000 / freq).max(1).into());
        let interval = interval(poll_period);
        let source = service.clone();
        let stream = interval_stream(interval).map(move |_| source.latest().mini_mono());
        let event = state(stream);
        Ok(Self { event, service })
    }

    pub fn bars(&self) -> [f32; MINI_BARS] { *self.event.borrow() }
    pub fn retry(&self) { self.service.retry(); }
    pub fn snapshot(&self) -> CavaSnapshot { self.service.latest() }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CavaConfig {
    pub framerate_hz: u32,
    pub bars: usize,
    pub channels: CavaChannels,
    pub reverse: bool,
}

#[derive(Debug, Clone, Copy)]
struct Control {
    desired: Option<CavaConfig>,
    retry_generation: u64,
    shutdown: bool,
}

struct ServiceInner {
    control: Arc<Mutex<Control>>,
    wake: SyncSender<()>,
    snapshot: Arc<Mutex<CavaSnapshot>>,
    error: Arc<Mutex<Option<String>>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

pub struct CavaService {
    inner: Arc<ServiceInner>,
}

impl Clone for CavaService {
    fn clone(&self) -> Self { Self { inner: Arc::clone(&self.inner) } }
}

impl CavaService {
    pub fn new() -> Self {
        let (wake, wake_rx) = mpsc::sync_channel(1);
        let snapshot = Arc::new(Mutex::new(CavaSnapshot::default()));
        let error = Arc::new(Mutex::new(None));
        let control = Arc::new(Mutex::new(Control { desired: None, retry_generation: 0, shutdown: false }));
        let worker_snapshot = Arc::clone(&snapshot);
        let worker_error = Arc::clone(&error);
        let worker_control = Arc::clone(&control);
        let thread = thread::spawn(move || service_worker(worker_control, wake_rx, worker_snapshot, worker_error));
        Self {
            inner: Arc::new(ServiceInner { control, wake, snapshot, error, thread: Mutex::new(Some(thread)) }),
        }
    }

    pub fn set_desired(&self, desired: Option<CavaConfig>) {
        let changed = {
            let mut control = self.inner.control.lock();
            if control.desired == desired {
                false
            } else {
                control.desired = desired;
                control.retry_generation = control.retry_generation.wrapping_add(1);
                *self.inner.error.lock() = None;
                true
            }
        };
        if changed { self.wake(); }

    }
    pub fn retry(&self) {
        {
            let mut control = self.inner.control.lock();
            control.retry_generation = control.retry_generation.wrapping_add(1);
            *self.inner.error.lock() = None;
        }
        self.wake();
    }
    pub fn shutdown(&self) {
        self.inner.control.lock().shutdown = true;
        self.wake();
    }

    pub fn shutdown_blocking(self) {
        self.shutdown();
        if Arc::strong_count(&self.inner) == 1 {
            if let Some(thread) = self.inner.thread.lock().take() {
                let _ = thread.join();
            }
        }
    }


    pub fn latest(&self) -> CavaSnapshot { *self.inner.snapshot.lock() }
    pub fn failure(&self) -> Option<String> { self.inner.error.lock().clone() }

    fn wake(&self) {
        match self.inner.wake.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        }
    }
}

impl Default for CavaService {
    fn default() -> Self { Self::new() }
}

impl Drop for CavaService {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) != 1 { return; }
        self.shutdown();
        let _ = self.inner.thread.lock().take();
    }
}

struct ActiveProcess {
    cfg: CavaConfig,
    child: Child,
    reader: Option<thread::JoinHandle<()>>,
    cfg_path: PathBuf,
}

fn service_worker(
    control: Arc<Mutex<Control>>,
    wake_rx: StdReceiver<()>,
    snapshot: Arc<Mutex<CavaSnapshot>>,
    error: Arc<Mutex<Option<String>>>,
) {
    let mut active: Option<ActiveProcess> = None;
    let mut failed_cfg: Option<CavaConfig> = None;
    let mut seen_retry = 0u64;
    loop {
        let _ = wake_rx.recv_timeout(Duration::from_millis(40));
        let ctl = *control.lock();
        if ctl.shutdown { break; }
        if ctl.retry_generation != seen_retry {
            seen_retry = ctl.retry_generation;
            failed_cfg = None;
        }

        if active.as_ref().map(|p| p.cfg) != ctl.desired {
            stop_process(&mut active);
            clear_snapshot(&snapshot);
        }
        let Some(desired) = ctl.desired else { continue };
        if active.is_none() && failed_cfg != Some(desired) {
            match spawn_process(desired, Arc::clone(&snapshot)) {
                Ok(process) => {
                    *error.lock() = None;
                    active = Some(process);
                }
                Err(err) => {
                    failed_cfg = Some(desired);
                    *error.lock() = Some(err.to_string());
                }
            }
        }
        let child_status = active.as_mut().map(|process| (process.cfg, process.child.try_wait()));
        match child_status {
            Some((cfg, Ok(Some(status)))) => {
                stop_process(&mut active);
                failed_cfg = Some(cfg);
                *error.lock() = Some(format!("cava exited with status {status}"));
            }
            Some((_, Ok(None))) | None => {}
            Some((cfg, Err(err))) => {
                stop_process(&mut active);
                failed_cfg = Some(cfg);
                *error.lock() = Some(format!("wait for cava: {err}"));
            }
        }
    }
    stop_process(&mut active);
}

fn clear_snapshot(snapshot: &Arc<Mutex<CavaSnapshot>>) {
    *snapshot.lock() = CavaSnapshot::default();

}

fn stop_process(active: &mut Option<ActiveProcess>) {
    let Some(mut process) = active.take() else { return };
    let _ = process.child.kill();
    let _ = process.child.wait();
    if let Some(reader) = process.reader.take() { let _ = reader.join(); }
    let _ = fs::remove_file(process.cfg_path);
}

fn spawn_process(cfg: CavaConfig, snapshot: Arc<Mutex<CavaSnapshot>>) -> Result<ActiveProcess> {
    let per_channel_bars = cfg.bars.clamp(1, MAX_BARS);
    let output_bars = match cfg.channels {
        CavaChannels::Stereo => per_channel_bars.saturating_mul(2).min(MAX_BARS * 2),
        CavaChannels::Mono => per_channel_bars,
    };
    let framerate_hz = cfg.framerate_hz.clamp(1, 120);
    let channels = match cfg.channels { CavaChannels::Stereo => "stereo", CavaChannels::Mono => "mono" };
    let reverse = if cfg.reverse { 1 } else { 0 };
    let text = format!(
        "[general]\nframerate = {framerate_hz}\nbars = {output_bars}\n\n[input]\n\n[output]\nmethod = raw\nchannels = {channels}\nreverse = {reverse}\nraw_target = /dev/stdout\ndata_format = ascii\nascii_max_range = 1000\nbar_delimiter = 59\nframe_delimiter = 10\n"
    );
    let (cfg_path, mut file) = create_temp_config()?;
    if let Err(err) = file.write_all(text.as_bytes()).and_then(|_| file.flush()) {
        let _ = fs::remove_file(&cfg_path);
        return Err(err).with_context(|| format!("write cava config: {}", cfg_path.display()));
    }
    drop(file);

    let cava_exe = match resolve_cava_executable() {
        Ok(path) => path,
        Err(err) => { let _ = fs::remove_file(&cfg_path); return Err(err); }
    };
    let mut child = match Command::new(&cava_exe)
        .arg("-p").arg(&cfg_path).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
    {
        Ok(child) => child,
        Err(err) => {
            let _ = fs::remove_file(&cfg_path);
            return Err(err).with_context(|| format!("spawn cava: {}", cava_exe.display()));
        }
    };
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&cfg_path);
            return Err(anyhow::anyhow!("failed to capture cava stdout"));
        }
    };
    let reader = thread::Builder::new().name("cava-output".into()).spawn(move || {
        read_output(stdout, cfg, snapshot);
    });
    let reader = match reader {
        Ok(reader) => reader,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = fs::remove_file(&cfg_path);
            return Err(err).context("spawn cava output reader");
        }
    };
    Ok(ActiveProcess { cfg, child, reader: Some(reader), cfg_path })
}

fn read_output(stdout: impl std::io::Read, cfg: CavaConfig, snapshot: Arc<Mutex<CavaSnapshot>>) {
    let mut br = BufReader::new(stdout);
    let mut line = String::new();
    let mut pending = [0.0f32; MAX_BARS];
    let mut has_pending = false;
    loop {
        line.clear();
        match br.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                let parsed = parse_ascii_line(&line, cfg.bars.clamp(1, MAX_BARS));
                if parsed.count == 0 { continue; }
                match cfg.channels {
                    CavaChannels::Mono => publish_snapshot(&snapshot, parsed.first, parsed.first, parsed.bars, false),
                    CavaChannels::Stereo if parsed.count >= 2 => {
                        publish_snapshot(&snapshot, parsed.first, parsed.second, parsed.bars, cfg.reverse);
                        has_pending = false;
                    }
                    CavaChannels::Stereo => {
                        if has_pending {
                            publish_snapshot(&snapshot, pending, parsed.first, parsed.bars, cfg.reverse);
                            has_pending = false;
                        } else {
                            pending = parsed.first;
                            has_pending = true;
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ParsedLine { count: u8, bars: usize, first: [f32; MAX_BARS], second: [f32; MAX_BARS] }

fn parse_ascii_line(s: &str, bars: usize) -> ParsedLine {
    let mut values = [0.0f32; MAX_BARS * 2];
    let mut len = 0usize;
    for part in s.split([';', '\n', '\r', ' ', '\t']) {
        if len == values.len() { break; }
        if let Ok(v) = part.parse::<u32>() {
            values[len] = (v as f32 / 1000.0).clamp(0.0, 1.0);
            len += 1;
        }
    }
    let mut out = ParsedLine { count: 0, bars, first: [0.0; MAX_BARS], second: [0.0; MAX_BARS] };
    if bars == 0 { return out; }
    if len >= bars * 2 {
        out.count = 2;
        out.first[..bars].copy_from_slice(&values[..bars]);
        out.second[..bars].copy_from_slice(&values[bars..bars * 2]);
    } else if len >= bars {
        out.count = 1;
        out.first[..bars].copy_from_slice(&values[..bars]);
    }
    out
}

fn publish_snapshot(
    snapshot: &Arc<Mutex<CavaSnapshot>>,
    mut left: [f32; MAX_BARS],
    mut right: [f32; MAX_BARS],
    bars: usize,
    reverse: bool,
) {
    canonicalize_channels(&mut left, &mut right, bars, reverse);
    let mut current = snapshot.lock();
    current.bars = bars.min(MAX_BARS);
    current.left = left;
    current.right = right;
}

fn canonicalize_channels(left: &mut [f32; MAX_BARS], right: &mut [f32; MAX_BARS], bars: usize, reverse: bool) {
    let bars = bars.min(MAX_BARS);
    // Cava's documented raw stereo layout is high->low on the left half and
    // low->high on the right half when reverse=0. Canonical snapshots keep
    // both channels low->high; reverse=1 swaps which side needs reversing.
    if reverse {
        right[..bars].reverse();
    } else {
        left[..bars].reverse();
    }
}
fn create_temp_config() -> Result<(PathBuf, fs::File)> {
    let pid = std::process::id();
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    for attempt in 0..32u32 {
        let path = std::env::temp_dir().join(format!("tmplayer-cava-{pid}-{ts}-{attempt}.conf"));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err).with_context(|| format!("create cava config: {}", path.display())),
        }
    }
    Err(anyhow::anyhow!("unable to allocate unique cava config path"))
}

fn find_cava_executable() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("TMPLAYER_CAVA") {
        let p = PathBuf::from(p);
        if p.is_file() { return Some(p); }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("cava"));
            candidates.push(exe_dir.join("third_party").join("cava").join("cava"));
        }
    }
    if let Ok(cwd) = std::env::current_dir() { candidates.push(cwd.join("third_party").join("cava").join("cava")); }
    for p in candidates { if p.is_file() { return Some(p); } }
    which_in_path("cava")
}

pub fn is_available() -> bool {
    static AVAILABLE: LazyLock<bool> = LazyLock::new(|| find_cava_executable().is_some());
    *AVAILABLE
}

fn resolve_cava_executable() -> Result<PathBuf> {
    find_cava_executable().ok_or_else(|| anyhow::anyhow!("cava not found"))
}

fn which_in_path(bin: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for p in std::env::split_paths(&paths) {
        let candidate = p.join(bin);
        if candidate.is_file() { return Some(candidate); }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combined_stereo_line() {
        let parsed = parse_ascii_line("100;200;300;400;500;600\n", 3);
        assert_eq!(parsed.count, 2);
        assert_eq!(parsed.first[..3], [0.1, 0.2, 0.3]);
        assert_eq!(parsed.second[..3], [0.4, 0.5, 0.6]);
    }

    #[test]
    fn parses_single_stereo_frame_without_mirroring() {
        let parsed = parse_ascii_line("100;900;300\n", 3);
        assert_eq!(parsed.count, 1);
        assert_eq!(parsed.first[..3], [0.1, 0.9, 0.3]);
    }

    #[test]
    fn canonicalizes_documented_stereo_orientation() {
        let mut left = [0.1; MAX_BARS];
        let mut right = [0.0; MAX_BARS];
        left[..3].copy_from_slice(&[0.1, 0.2, 0.3]);
        right[..3].copy_from_slice(&[0.4, 0.5, 0.6]);
        canonicalize_channels(&mut left, &mut right, 3, false);
        assert_eq!(left[..3], [0.3, 0.2, 0.1]);
        assert_eq!(right[..3], [0.4, 0.5, 0.6]);
    }

    #[cfg(unix)]
    #[test]
    fn failed_process_is_single_flight_until_explicit_retry() {
        use std::os::unix::fs::PermissionsExt;

        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir();
        let config_prefix = format!("tmplayer-cava-{}-", std::process::id());
        let config_before: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|entry| entry.path()).filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(&config_prefix))).collect();
        let script = dir.join(format!("tmplayer-cava-test-{nonce}.sh"));
        let marker = dir.join(format!("tmplayer-cava-test-{nonce}.count"));
        let script_text = format!("#!/bin/sh\nprintf x >> '{}'\nexit 17\n", marker.display());
        fs::write(&script, script_text).unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).unwrap();
        let previous = std::env::var_os("TMPLAYER_CAVA");
        unsafe { std::env::set_var("TMPLAYER_CAVA", &script); }

        let service = CavaService::new();
        service.set_desired(Some(CavaConfig { framerate_hz: 30, bars: 3, channels: CavaChannels::Stereo, reverse: false }));
        thread::sleep(Duration::from_millis(140));
        assert!(service.failure().is_some());
        let first_count = fs::read_to_string(&marker).unwrap_or_default().len();
        assert_eq!(first_count, 1);
        thread::sleep(Duration::from_millis(100));
        assert_eq!(fs::read_to_string(&marker).unwrap_or_default().len(), 1);

        service.retry();
        thread::sleep(Duration::from_millis(140));
        assert_eq!(fs::read_to_string(&marker).unwrap_or_default().len(), 2);
        service.shutdown_blocking();

        if let Some(value) = previous {
            unsafe { std::env::set_var("TMPLAYER_CAVA", value); }
        } else {
            unsafe { std::env::remove_var("TMPLAYER_CAVA"); }
        }
        let config_after: Vec<_> = fs::read_dir(&dir).unwrap().filter_map(Result::ok).map(|entry| entry.path()).filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with(&config_prefix))).collect();
        assert_eq!(config_after, config_before);
        let _ = fs::remove_file(script);
        let _ = fs::remove_file(marker);
    }
}
