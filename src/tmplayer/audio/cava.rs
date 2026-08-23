use crate::state;
use compio::time::{Interval, interval};
use futures::stream::unfold;
use futures::{Stream, StreamExt};
use see::unsync::Receiver;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CavaChannels {
    Stereo,
    Mono,
}

#[derive(Default)]
pub(crate) struct AnalyzerBuffer {
    samples: VecDeque<f32>,
    sample_rate: u32,
}

pub type SharedAnalyzer = Arc<Mutex<AnalyzerBuffer>>;

fn analyzer() -> &'static SharedAnalyzer {
    static ANALYZER: OnceLock<SharedAnalyzer> = OnceLock::new();
    ANALYZER.get_or_init(|| Arc::new(Mutex::new(AnalyzerBuffer::default())))
}

pub fn shared_analyzer() -> SharedAnalyzer {
    Arc::clone(analyzer())
}

pub struct MiniCavaState {
    pub event: Receiver<[f32; 20]>,
}

pub struct CavaRunner {
    analyzer: SharedAnalyzer,
    cfg: CavaConfig,
}

impl CavaRunner {
    pub fn start(cfg: CavaConfig) -> anyhow::Result<Self> {
        Ok(Self {
            analyzer: shared_analyzer(),
            cfg,
        })
    }

    pub fn latest_bars(&self) -> Vec<f32> {
        let base = analyze(&self.analyzer, self.cfg);
        let count = self.cfg.bars.clamp(8, 96);
        (0..count)
            .map(|index| {
                let position = index as f32 * 19.0 / (count.saturating_sub(1).max(1) as f32);
                let low = position.floor() as usize;
                let high = position.ceil() as usize;
                let fraction = position.fract();
                base[low.min(19)] * (1.0 - fraction) + base[high.min(19)] * fraction
            })
            .collect()
    }

    pub fn latest_stereo_bars(&self) -> (Vec<f32>, Vec<f32>) {
        let bars = self.latest_bars();
        (bars.clone(), bars)
    }
}

fn interval_stream(interval: Interval) -> impl Stream<Item = Instant> {
    unfold(interval, async |mut interval| {
        Some((interval.tick().await, interval))
    })
}

impl MiniCavaState {
    pub fn try_new(cfg: CavaConfig) -> anyhow::Result<Self> {
        let period = Duration::from_millis((1000 / cfg.framerate_hz.clamp(10, 120)).into());
        let analyzer = shared_analyzer();
        let stream = interval_stream(interval(period)).map(move |_| analyze(&analyzer, cfg));
        Ok(Self {
            event: state(stream),
        })
    }

    pub fn bars(&self) -> [f32; 20] {
        *self.event.borrow()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CavaConfig {
    pub framerate_hz: u32,
    pub bars: usize,
    pub channels: CavaChannels,
    pub reverse: bool,
}

pub fn is_available() -> bool {
    true
}

fn analyze(analyzer: &SharedAnalyzer, cfg: CavaConfig) -> [f32; 20] {
    let (samples, sample_rate) = {
        let guard = analyzer.lock().unwrap();
        let take = guard.samples.len().min(1024);
        (
            guard
                .samples
                .iter()
                .skip(guard.samples.len().saturating_sub(take))
                .copied()
                .collect::<Vec<_>>(),
            guard.sample_rate.max(8_000) as f32,
        )
    };
    if samples.len() < 64 {
        return [0.0; 20];
    }

    let n = samples.len() as f32;
    let bands = cfg.bars.clamp(8, 20).min(20);
    let mut output = [0.0f32; 20];
    for band in 0..bands {
        let t = (band as f32 + 0.5) / bands as f32;
        let max_frequency = (sample_rate * 0.45).max(500.0);
        let frequency = 55.0_f32 * (max_frequency / 55.0_f32).powf(t);
        let omega = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        let mut real = 0.0;
        let mut imag = 0.0;
        for (index, sample) in samples.iter().enumerate() {
            let window =
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * index as f32 / (n - 1.0).max(1.0)).cos();
            let phase = omega * index as f32;
            real += sample * window * phase.cos();
            imag -= sample * window * phase.sin();
        }
        let magnitude = (real * real + imag * imag).sqrt() / n;
        output[if cfg.reverse { bands - 1 - band } else { band }] =
            (magnitude * 5.0).sqrt().clamp(0.0, 1.0);
    }
    output
}

pub fn record_sample(sample: f32, sample_rate: u32, channel_count: u16, channel: usize) {
    let mut guard = analyzer().lock().unwrap();
    guard.sample_rate = sample_rate;
    if channel_count > 1 && channel != 0 {
        return;
    }
    guard.samples.push_back(sample.clamp(-1.0, 1.0));
    while guard.samples.len() > 2048 {
        guard.samples.pop_front();
    }
}

#[cfg(test)]
mod tests {
    use super::{AnalyzerBuffer, CavaChannels, CavaConfig, SharedAnalyzer, analyze};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[test]
    fn silence_does_not_create_spectrum() {
        let analyzer: SharedAnalyzer = Arc::new(Mutex::new(AnalyzerBuffer {
            samples: VecDeque::from(vec![0.0; 512]),
            sample_rate: 5_512,
        }));
        let bars = analyze(
            &analyzer,
            CavaConfig {
                framerate_hz: 30,
                bars: 20,
                channels: CavaChannels::Mono,
                reverse: false,
            },
        );
        assert!(bars.iter().all(|value| *value == 0.0));
    }
}
