#[derive(Debug, Clone)]
pub struct Ema {
    alpha: f32,
    state: Vec<f32>,
}

impl Ema {
    pub fn new(alpha: f32, len: usize) -> Self {
        Self {
            alpha: alpha.clamp(0.0, 1.0),
            state: vec![0.0; len],
        }
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
}

#[cfg(test)]
mod tests {
    use super::Ema;

    #[test]
    fn smooths_into_reused_output_and_resets_between_streams() {
        let mut smoother = Ema::new(0.35, 2);
        let mut output = [0.0; 2];
        assert_eq!(smoother.apply_in_place(&[1.0, 0.0], &mut output), 2);
        assert_eq!(output, [0.35, 0.0]);
        smoother.apply_in_place(&[1.0, 1.0], &mut output);
        assert!((output[0] - 0.5775).abs() < 0.00001);
        assert_eq!(output[1], 0.35);
        smoother.reset();
        smoother.apply_in_place(&[0.0, 1.0], &mut output);
        assert_eq!(output, [0.0, 0.35]);
    }

    #[test]
    fn stereo_channels_keep_independent_history() {
        let mut left = Ema::new(0.35, 1);
        let mut right = Ema::new(0.35, 1);
        let (mut left_out, mut right_out) = ([0.0], [0.0]);
        left.apply_in_place(&[1.0], &mut left_out);
        right.apply_in_place(&[0.0], &mut right_out);
        assert_eq!(left_out, [0.35]);
        assert_eq!(right_out, [0.0]);
        left.apply_in_place(&[0.0], &mut left_out);
        right.apply_in_place(&[1.0], &mut right_out);
        assert!((left_out[0] - 0.2275).abs() < 0.00001);
        assert_eq!(right_out, [0.35]);
    }
}
