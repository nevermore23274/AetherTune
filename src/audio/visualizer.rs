use rand::Rng;

use crate::audio::pipe::{SharedAnalysis, NUM_BANDS};

const NUM_BARS: usize = NUM_BANDS; // Match the frequency bands
const MAX_HEIGHT: u16 = 12;

/// Noise reduction factor (0.0 = fast/noisy, 1.0 = slow/smooth)
/// Controls both integral smoothing weight and gravity modifier.
/// CAVA default is 0.77; we use 0.70 for slightly more responsiveness in a TUI.
const NOISE_REDUCTION: f64 = 0.70;

/// Gravity acceleration increment per frame when a bar is falling.
/// CAVA uses 0.028; we match that.
const GRAVITY_STEP: f64 = 0.028;

/// How fast autosens decreases when bars overshoot (2% drop per frame)
const AUTOSENS_DECREASE: f64 = 0.98;
/// How fast autosens increases when bars are in range (0.1% rise per frame)
const AUTOSENS_INCREASE: f64 = 1.001;
/// Fast initial ramp-up multiplier when first sounds appear
const AUTOSENS_INIT_INCREASE: f64 = 1.05;

pub struct Visualizer {
    pub bars: Vec<u16>,
    pub peaks: Vec<u16>,
    active: bool,
    frame: u64,

    // --- CAVA-style processing state ---

    /// Previous output values (for gravity comparison)
    prev_out: Vec<f64>,
    /// Peak values for gravity fall-off tracking
    cava_peak: Vec<f64>,
    /// Per-bar fall acceleration (increases each frame the bar is falling)
    cava_fall: Vec<f64>,
    /// Integral smoothing memory (weighted running average)
    cava_mem: Vec<f64>,

    /// Automatic sensitivity multiplier — dynamically adjusts so bars
    /// fill the 0..1 range without constant clipping or silence
    sensitivity: f64,
    /// Whether we're still in the initial ramp-up phase
    sens_init: bool,
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            bars: vec![0; NUM_BARS],
            peaks: vec![0; NUM_BARS],
            active: false,
            frame: 0,
            prev_out: vec![0.0; NUM_BARS],
            cava_peak: vec![0.0; NUM_BARS],
            cava_fall: vec![0.0; NUM_BARS],
            cava_mem: vec![0.0; NUM_BARS],
            sensitivity: 1.0,
            sens_init: true,
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

        // Gravity modifier: higher noise_reduction = lower gravity = slower fall
        let gravity_mod = 1.0 - (NOISE_REDUCTION * 0.5);

        let mut overshoot = false;
        let silence = rms < 0.001;

        for i in 0..NUM_BARS {
            // Raw input: band energy scaled by RMS and volume, then sensitivity
            let raw = bands[i] * rms.sqrt().min(1.0) * 3.0 * vol_scale;
            let mut out = raw * self.sensitivity;

            // --- Gravity fall-off ---
            // If the new value is below the previous peak, apply accelerating
            // gravity instead of jumping down immediately
            if out < self.prev_out[i] {
                out = self.cava_peak[i]
                    * (1.0 - (self.cava_fall[i] * self.cava_fall[i] * gravity_mod));
                if out < 0.0 {
                    out = 0.0;
                }
                self.cava_fall[i] += GRAVITY_STEP;
            } else {
                // New peak — reset fall tracking
                self.cava_peak[i] = out;
                self.cava_fall[i] = 0.0;
            }
            self.prev_out[i] = out;

            // --- Integral smoothing ---
            // Weighted running average: memory accumulates over time,
            // blending the previous memory with the current value
            out = self.cava_mem[i] * NOISE_REDUCTION + out;
            self.cava_mem[i] = out;

            // --- Autosens clamping ---
            if out > 1.0 {
                overshoot = true;
                out = 1.0;
            }

            // Convert to bar height
            let height = (out * MAX_HEIGHT as f64) as u16;
            self.bars[i] = height.min(MAX_HEIGHT);

            // Peak hold with slow probabilistic decay
            if self.bars[i] >= self.peaks[i] {
                self.peaks[i] = self.bars[i];
            } else if rng.random_range(0..6) == 0 {
                self.peaks[i] = self.peaks[i].saturating_sub(1);
            }
        }

        // --- Automatic sensitivity adjustment ---
        // When bars overshoot: decrease quickly to avoid clipping
        // When bars are in range: increase slowly to fill the display
        if overshoot {
            self.sensitivity *= AUTOSENS_DECREASE;
            self.sens_init = false;
        } else if !silence {
            if self.sens_init {
                // Fast ramp-up at the start so bars appear quickly
                self.sensitivity *= AUTOSENS_INIT_INCREASE;
            } else {
                self.sensitivity *= AUTOSENS_INCREASE;
            }
        }

        true
    }

    /// Tick using simulated data (fallback when no FIFO pipe)
    pub fn tick_simulated(&mut self, is_playing: bool, audio_level: f64, volume: u32) {
        self.frame = self.frame.wrapping_add(1);

        if !is_playing {
            // Apply gravity fall-off even in simulated mode for smooth stop
            let gravity_mod = 1.0 - (NOISE_REDUCTION * 0.5);
            for i in 0..NUM_BARS {
                if self.bars[i] > 0 {
                    self.cava_fall[i] += GRAVITY_STEP;
                    let peak = self.cava_peak[i];
                    let fallen = peak
                        * (1.0 - (self.cava_fall[i] * self.cava_fall[i] * gravity_mod));
                    let height = if fallen > 0.0 {
                        (fallen * MAX_HEIGHT as f64) as u16
                    } else {
                        0
                    };
                    self.bars[i] = height.min(MAX_HEIGHT);
                }
                if self.peaks[i] > 0 {
                    self.peaks[i] = self.peaks[i].saturating_sub(1);
                }
            }
            self.active = false;
            return;
        }

        self.active = true;
        let mut rng = rand::rng();
        let vol_scale = volume as f64 / 100.0;
        let gravity_mod = 1.0 - (NOISE_REDUCTION * 0.5);

        if self.frame % 3 == 0 {
            let has_real_data = audio_level > 0.01;

            for i in 0..NUM_BARS {
                let raw = if has_real_data {
                    let base = audio_level;
                    let spread: f64 = rng.random_range(-0.15..=0.15);
                    (base + spread).clamp(0.05, 1.0) * vol_scale
                } else {
                    let t = self.frame as f64 * 0.08;
                    let bar_offset = i as f64;
                    let w1 = ((t + bar_offset * 0.4).sin() + 1.0) / 2.0;
                    let w2 = ((t * 0.6 + bar_offset * 0.9).sin() + 1.0) / 2.0;
                    let w3 = ((t * 1.3 + bar_offset * 0.2).cos() + 1.0) / 2.0;
                    let combined = w1 * 0.5 + w2 * 0.3 + w3 * 0.2;
                    (combined * vol_scale).clamp(0.0, 1.0)
                };

                let mut out = raw;

                // Gravity fall-off
                if out < self.prev_out[i] {
                    out = self.cava_peak[i]
                        * (1.0 - (self.cava_fall[i] * self.cava_fall[i] * gravity_mod));
                    if out < 0.0 {
                        out = 0.0;
                    }
                    self.cava_fall[i] += GRAVITY_STEP;
                } else {
                    self.cava_peak[i] = out;
                    self.cava_fall[i] = 0.0;
                }
                self.prev_out[i] = out;

                // Integral smoothing
                out = self.cava_mem[i] * NOISE_REDUCTION + out;
                self.cava_mem[i] = out;

                out = out.min(1.0);

                let height = (out * MAX_HEIGHT as f64) as u16;
                self.bars[i] = height.min(MAX_HEIGHT);
            }
        }

        for i in 0..NUM_BARS {
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
        self.prev_out = vec![0.0; NUM_BARS];
        self.cava_peak = vec![0.0; NUM_BARS];
        self.cava_fall = vec![0.0; NUM_BARS];
        self.cava_mem = vec![0.0; NUM_BARS];
        self.sensitivity = 1.0;
        self.sens_init = true;
        self.active = false;
    }

    pub fn num_bars(&self) -> usize {
        NUM_BARS
    }

    pub fn max_height(&self) -> u16 {
        MAX_HEIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_visualizer_is_zeroed() {
        let vis = Visualizer::new();
        assert!(vis.bars.iter().all(|&b| b == 0));
        assert!(vis.peaks.iter().all(|&p| p == 0));
        assert_eq!(vis.sensitivity, 1.0);
        assert!(vis.sens_init);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut vis = Visualizer::new();
        vis.bars[0] = 10;
        vis.peaks[0] = 12;
        vis.sensitivity = 2.5;
        vis.cava_mem[0] = 0.8;
        vis.cava_fall[0] = 0.5;
        vis.sens_init = false;

        vis.reset();

        assert!(vis.bars.iter().all(|&b| b == 0));
        assert!(vis.peaks.iter().all(|&p| p == 0));
        assert_eq!(vis.sensitivity, 1.0);
        assert!(vis.sens_init);
        assert!(vis.cava_mem.iter().all(|&m| m == 0.0));
        assert!(vis.cava_fall.iter().all(|&f| f == 0.0));
    }

    #[test]
    fn test_simulated_not_playing_decays() {
        let mut vis = Visualizer::new();
        // Set some initial bar heights and peaks
        for i in 0..NUM_BARS {
            vis.bars[i] = 8;
            vis.peaks[i] = 10;
            vis.cava_peak[i] = 0.7;
            vis.cava_fall[i] = 0.0;
        }

        // Tick several times with is_playing=false
        for _ in 0..50 {
            vis.tick_simulated(false, 0.0, 50);
        }

        // All bars should have decayed toward zero
        assert!(
            vis.bars.iter().all(|&b| b < 4),
            "Bars should decay when not playing, got: {:?}",
            vis.bars
        );
    }

    #[test]
    fn test_gravity_constants_are_sane() {
        assert!(NOISE_REDUCTION > 0.0 && NOISE_REDUCTION < 1.0);
        assert!(GRAVITY_STEP > 0.0 && GRAVITY_STEP < 0.1);
        assert!(AUTOSENS_DECREASE < 1.0);
        assert!(AUTOSENS_INCREASE > 1.0);
        assert!(AUTOSENS_INIT_INCREASE > AUTOSENS_INCREASE);
    }

    #[test]
    fn test_num_bars_and_max_height() {
        let vis = Visualizer::new();
        assert_eq!(vis.num_bars(), NUM_BANDS);
        assert_eq!(vis.max_height(), 12);
    }
}