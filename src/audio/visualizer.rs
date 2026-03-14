use rand::Rng;

use crate::audio::pipe::{SharedAnalysis, NUM_BANDS};

const NUM_BARS: usize = NUM_BANDS; // Match the frequency bands
const MAX_HEIGHT: u16 = 12;

pub struct Visualizer {
    pub bars: Vec<u16>,
    pub peaks: Vec<u16>,
    targets: Vec<u16>,
    active: bool,
    frame: u64,
    /// Smoothed band values for interpolation
    smooth_bands: Vec<f64>,
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            bars: vec![0; NUM_BARS],
            peaks: vec![0; NUM_BARS],
            targets: vec![0; NUM_BARS],
            active: false,
            frame: 0,
            smooth_bands: vec![0.0; NUM_BARS],
        }
    }

    /// Tick using real audio analysis data from the FIFO pipe.
    /// Returns true if real data was used.
    pub fn tick_real(&mut self, analysis: &SharedAnalysis, volume: u32) -> bool {
        self.frame = self.frame.wrapping_add(1);

        let (active, bands, rms) = {
            if let Ok(a) = analysis.lock() {
                (a.active, a.bands, a.rms)
            } else {
                return false;
            }
        };

        if !active {
            return false;
        }

        self.active = true;
        let mut rng = rand::rng();
        let vol_scale = volume as f64 / 100.0;

        // Smooth the band values (exponential moving average)
        // Fast attack, slower decay — feels more responsive
        for i in 0..NUM_BARS {
            let target = bands[i] * rms.sqrt().min(1.0) * 3.0; // Scale up for visibility
            let target = (target * vol_scale).min(1.0);

            if target > self.smooth_bands[i] {
                // Fast attack
                self.smooth_bands[i] = self.smooth_bands[i] * 0.3 + target * 0.7;
            } else {
                // Slower decay
                self.smooth_bands[i] = self.smooth_bands[i] * 0.7 + target * 0.3;
            }

            // Convert to bar height
            let height = (self.smooth_bands[i] * MAX_HEIGHT as f64) as u16;
            self.targets[i] = height.min(MAX_HEIGHT);
        }

        // Interpolate bars toward targets
        for i in 0..NUM_BARS {
            let current = self.bars[i];
            let target = self.targets[i];

            if target > current {
                self.bars[i] = current + (target - current).min(3); // Faster rise for real data
            } else if target < current {
                self.bars[i] = current.saturating_sub(1);
            }

            // Peak hold
            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
            } else if rng.random_range(0..6) == 0 {
                self.peaks[i] = self.peaks[i].saturating_sub(1);
            }
        }

        true
    }

    /// Tick using simulated data (fallback when no FIFO pipe)
    pub fn tick_simulated(&mut self, is_playing: bool, audio_level: f64, volume: u32) {
        self.frame = self.frame.wrapping_add(1);

        if !is_playing {
            for bar in &mut self.bars {
                *bar = bar.saturating_sub(1);
            }
            for peak in &mut self.peaks {
                *peak = peak.saturating_sub(1);
            }
            self.active = false;
            return;
        }

        self.active = true;
        let mut rng = rand::rng();
        let vol_scale = volume as f64 / 100.0;

        if self.frame % 3 == 0 {
            let has_real_data = audio_level > 0.01;

            for i in 0..NUM_BARS {
                if has_real_data {
                    let base = (audio_level * MAX_HEIGHT as f64) as i32;
                    let spread: i32 = rng.random_range(-3..=3);
                    self.targets[i] = (base + spread).clamp(1, MAX_HEIGHT as i32) as u16;
                } else {
                    let t = self.frame as f64 * 0.08;
                    let bar_offset = i as f64;

                    let w1 = ((t + bar_offset * 0.4).sin() + 1.0) / 2.0;
                    let w2 = ((t * 0.6 + bar_offset * 0.9).sin() + 1.0) / 2.0;
                    let w3 = ((t * 1.3 + bar_offset * 0.2).cos() + 1.0) / 2.0;

                    let combined = w1 * 0.5 + w2 * 0.3 + w3 * 0.2;
                    let height = (combined * vol_scale * MAX_HEIGHT as f64) as i32;
                    let jitter: i32 = rng.random_range(-1..=1);
                    self.targets[i] = (height + jitter).clamp(1, MAX_HEIGHT as i32) as u16;
                }
            }
        }

        for i in 0..NUM_BARS {
            let current = self.bars[i];
            let target = self.targets[i];

            if target > current {
                self.bars[i] = current + (target - current).min(2);
            } else if target < current {
                self.bars[i] = current.saturating_sub(1);
            }

            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
            } else if rng.random_range(0..8) == 0 {
                self.peaks[i] = self.peaks[i].saturating_sub(1);
            }
        }
    }

    pub fn reset(&mut self) {
        self.bars = vec![0; NUM_BARS];
        self.peaks = vec![0; NUM_BARS];
        self.targets = vec![0; NUM_BARS];
        self.smooth_bands = vec![0.0; NUM_BARS];
        self.active = false;
    }

    pub fn num_bars(&self) -> usize {
        NUM_BARS
    }

    pub fn max_height(&self) -> u16 {
        MAX_HEIGHT
    }
}