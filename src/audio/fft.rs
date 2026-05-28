use super::pipe::NUM_BANDS;

/// FFT window size — must be a power of 2
pub const FFT_SIZE: usize = 1024;
/// Usable frequency bins (first half of FFT output)
pub const MAGNITUDE_COUNT: usize = FFT_SIZE / 2;

/// In-place radix-2 Cooley-Tukey FFT.
///
/// Input length must be a power of 2. Operates on separate real/imag
/// arrays to avoid complex number overhead and heap allocation.
pub fn fft_in_place(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two(), "FFT size must be a power of 2");
    debug_assert_eq!(re.len(), im.len());

    // Bit-reversal permutation
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }

    // Butterfly passes
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = -2.0 * std::f64::consts::PI / len as f64;
        let wn_re = angle.cos();
        let wn_im = angle.sin();

        let mut i = 0;
        while i < n {
            let mut w_re = 1.0;
            let mut w_im = 0.0;

            for k in 0..half {
                let a = i + k;
                let b = a + half;

                // Complex multiply: t = w * data[b]
                let t_re = w_re * re[b] - w_im * im[b];
                let t_im = w_re * im[b] + w_im * re[b];

                // Butterfly
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;

                // Advance twiddle factor: w *= wn
                let new_w_re = w_re * wn_re - w_im * wn_im;
                w_im = w_re * wn_im + w_im * wn_re;
                w_re = new_w_re;
            }

            i += len;
        }
        len <<= 1;
    }
}

/// Pre-compute logarithmic band edges as bin indices.
/// Returns NUM_BANDS + 1 edge values.
pub fn compute_band_edges() -> Vec<usize> {
    // From ~50Hz to ~10kHz (CAVA recommended range for music visualization)
    let min_freq: f64 = 50.0;
    let max_freq: f64 = 10000.0;
    let sample_rate: f64 = 48000.0;
    let freq_resolution = sample_rate / FFT_SIZE as f64; // ~46.875 Hz

    (0..=NUM_BANDS)
        .map(|i| {
            let t = i as f64 / NUM_BANDS as f64;
            let freq = min_freq * (max_freq / min_freq).powf(t);
            let bin = (freq / freq_resolution).round() as usize;
            bin.max(1).min(MAGNITUDE_COUNT)
        })
        .collect()
}

/// Group FFT magnitudes into NUM_BANDS logarithmically-spaced bands
/// with perceptual weighting.
pub fn group_into_bands(magnitudes: &[f64], band_edges: &[usize]) -> [f64; NUM_BANDS] {
    let mut energies = [0.0f64; NUM_BANDS];

    for band in 0..NUM_BANDS {
        let bin_start = band_edges[band];
        let mut bin_end = band_edges[band + 1];
        if bin_end <= bin_start {
            bin_end = bin_start + 1;
        }
        if bin_end > magnitudes.len() {
            bin_end = magnitudes.len();
        }

        // Average all bins in this band — with a full FFT we have every bin,
        // no need to subsample like the old partial DFT
        let mut sum = 0.0;
        for i in bin_start..bin_end {
            sum += magnitudes[i];
        }
        energies[band] = sum / (bin_end - bin_start) as f64;
    }

    // Normalize: find max energy and scale so the loudest band is ~1.0
    // Apply perceptual weighting — boost higher bands more aggressively
    // since energy naturally drops off with frequency in most music.
    let max_e = energies.iter().cloned().fold(0.0f64, f64::max);
    if max_e > 0.0001 {
        for i in 0..NUM_BANDS {
            // Progressive boost: ~1.0x at band 0 (50Hz), ~2.0x at band 15 (10kHz)
            let boost = 1.0 + (i as f64 / NUM_BANDS as f64) * 1.0;
            energies[i] = (energies[i] / max_e * boost).min(1.0);
        }
    }

    energies
}

/// Legacy public interface — compute band energies from raw samples.
/// Used by tests and potentially external callers.
pub fn compute_band_energies(samples: &[f64], n: usize) -> [f64; NUM_BANDS] {
    let mut re: Vec<f64> = samples.iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64).cos());
            s * w
        })
        .collect();
    let mut im = vec![0.0f64; n];

    // Pad to power of 2 if needed
    let fft_len = n.next_power_of_two();
    re.resize(fft_len, 0.0);
    im.resize(fft_len, 0.0);

    fft_in_place(&mut re, &mut im);

    let mag_count = fft_len / 2;
    let magnitudes: Vec<f64> = (0..mag_count)
        .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt() / fft_len as f64)
        .collect();

    let band_edges = compute_band_edges();
    group_into_bands(&magnitudes, &band_edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// Generate a sine wave at a given frequency
    fn sine_wave(freq: f64, sample_rate: f64, num_samples: usize) -> Vec<f64> {
        (0..num_samples)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (2.0 * PI * freq * t).sin()
            })
            .collect()
    }

    #[test]
    fn test_fft_silence() {
        let samples = vec![0.0f64; 1024];
        let energies = compute_band_energies(&samples, 1024);
        assert!(
            energies.iter().all(|&e| e < 0.001),
            "Silent input should produce near-zero energy in all bands"
        );
    }

    #[test]
    fn test_fft_single_tone_440hz() {
        let samples = sine_wave(440.0, 48000.0, 1024);
        let energies = compute_band_energies(&samples, 1024);

        // 440Hz falls in a low-mid band. Find which band has the peak.
        let max_band = energies
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // 440Hz should land roughly in bands 4-7 (logarithmic spacing from 50-10000Hz, 16 bands)
        assert!(
            max_band >= 3 && max_band <= 9,
            "440Hz peak in band {} — expected roughly bands 3-9",
            max_band
        );
    }

    #[test]
    fn test_fft_high_tone_vs_low_tone() {
        let low = sine_wave(100.0, 48000.0, 1024);
        let high = sine_wave(8000.0, 48000.0, 1024);

        let low_energies = compute_band_energies(&low, 1024);
        let high_energies = compute_band_energies(&high, 1024);

        let low_peak = low_energies
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        let high_peak = high_energies
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        assert!(
            high_peak > low_peak,
            "8kHz peak (band {}) should be in a higher band than 100Hz peak (band {})",
            high_peak, low_peak
        );
    }

    #[test]
    fn test_fft_known_frequency() {
        // A pure 1000Hz tone at 48kHz sample rate, 1024 samples.
        // Bin k = freq * N / sample_rate = 1000 * 1024 / 48000 ≈ 21.33
        // So bin 21 should have the highest magnitude.
        let samples = sine_wave(1000.0, 48000.0, 1024);
        let mut re: Vec<f64> = samples.iter()
            .enumerate()
            .map(|(i, &s)| {
                let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / 1023.0).cos());
                s * w
            })
            .collect();
        let mut im = vec![0.0f64; 1024];

        fft_in_place(&mut re, &mut im);

        let magnitudes: Vec<f64> = (0..512)
            .map(|i| (re[i] * re[i] + im[i] * im[i]).sqrt())
            .collect();

        let peak_bin = magnitudes
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap();

        // Should be near bin 21 (1000 * 1024 / 48000 ≈ 21.3)
        assert!(
            peak_bin >= 20 && peak_bin <= 23,
            "1kHz tone peak at bin {} — expected ~21",
            peak_bin
        );
    }

    #[test]
    fn test_fft_roundtrip() {
        // FFT then IFFT should recover the original signal (within floating point error)
        let original = sine_wave(440.0, 48000.0, 1024);
        let mut re = original.clone();
        let mut im = vec![0.0f64; 1024];

        // Forward FFT
        fft_in_place(&mut re, &mut im);

        // Manual inverse: conjugate, FFT, conjugate, divide by N
        for v in im.iter_mut() {
            *v = -*v;
        }
        fft_in_place(&mut re, &mut im);
        for v in im.iter_mut() {
            *v = -*v;
        }
        let n = 1024.0;
        for i in 0..1024 {
            re[i] /= n;
            im[i] /= n;
        }

        // Check that real parts match the original, imaginary parts are ~0
        for i in 0..1024 {
            assert!(
                (re[i] - original[i]).abs() < 1e-10,
                "Roundtrip mismatch at sample {}: {} vs {}",
                i, re[i], original[i]
            );
            assert!(
                im[i].abs() < 1e-10,
                "Imaginary part should be ~0 at sample {}: {}",
                i, im[i]
            );
        }
    }

    #[test]
    fn test_band_count() {
        let samples = sine_wave(1000.0, 48000.0, 1024);
        let energies = compute_band_energies(&samples, 1024);
        assert_eq!(energies.len(), NUM_BANDS);
    }
}