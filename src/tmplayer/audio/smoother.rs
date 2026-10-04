#[derive(Debug, Clone)]
pub struct Ema {
    alpha: f32,
    state: Vec<f32>,
}

impl Ema {
    pub fn new(alpha: f32, len: usize) -> Self {
        Self { alpha: alpha.clamp(0.0, 1.0), state: vec![0.0; len] }
    }

    pub fn reset(&mut self) {
        self.state.fill(0.0);
    }

    /// Smooth `input` directly into a caller-owned output buffer.
    ///
    /// The state is resized only when the stream shape changes; steady-state
    /// rendering performs no allocation or copying of the returned samples.
    pub fn apply_in_place(&mut self, input: &[f32], output: &mut [f32]) -> usize {
        if self.state.len() != input.len() {
            self.state.resize(input.len(), 0.0);
        }
        let n = input.len().min(output.len());
        for (i, (src, dst)) in input[..n].iter().zip(output[..n].iter_mut()).enumerate() {
            let value = self.alpha * *src + (1.0 - self.alpha) * self.state[i];
            self.state[i] = value;
            *dst = value;
        }
        n
    }

    pub fn len(&self) -> usize { self.state.len() }
    pub fn is_empty(&self) -> bool { self.state.is_empty() }
}
