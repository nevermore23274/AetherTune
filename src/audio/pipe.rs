#[cfg(unix)]
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Number of frequency bands we compute for the visualizer
pub const NUM_BANDS: usize = 16;

/// FFT window size — must be a power of 2
const FFT_SIZE: usize = 1024;
/// Usable frequency bins (first half of FFT output)
const MAGNITUDE_COUNT: usize = FFT_SIZE / 2;

/// Shared state between the reader thread and the main thread
pub struct AudioAnalysis {
    /// Per-band energy levels, 0.0..1.0
    pub bands: [f64; NUM_BANDS],
    /// Overall RMS level, 0.0..1.0
    pub rms: f64,
    /// Whether the reader is actively receiving data
    pub active: bool,
}

impl AudioAnalysis {
    fn new() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            rms: 0.0,
            active: false,
        }
    }
}

pub type SharedAnalysis = Arc<Mutex<AudioAnalysis>>;

/// Create the shared analysis state
pub fn new_shared_analysis() -> SharedAnalysis {
    Arc::new(Mutex::new(AudioAnalysis::new()))
}

/// The FIFO path for this process
#[cfg(unix)]
pub fn fifo_path() -> PathBuf {
    std::env::temp_dir().join(format!("aethertune-pcm-{}", std::process::id()))
}

/// Create the named FIFO pipe. Returns true if successful.
#[cfg(unix)]
pub fn create_fifo(path: &std::path::Path) -> bool {
    // Remove any stale FIFO
    let _ = std::fs::remove_file(path);

    let path_cstr = match std::ffi::CString::new(path.to_string_lossy().as_bytes()) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // mkfifo via libc
    let ret = unsafe { libc::mkfifo(path_cstr.as_ptr(), 0o644) };
    ret == 0
}

/// Spawn a background thread that reads raw PCM s16le/stereo/48kHz from the FIFO
/// and computes spectral band data into the shared analysis state.
///
/// The thread will block on opening the FIFO until a writer connects (mpv).
/// It runs until the FIFO is closed (mpv stops) or the thread is dropped.
#[cfg(unix)]
pub fn spawn_reader(fifo: PathBuf, analysis: SharedAnalysis) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        reader_loop(&fifo, &analysis);
    })
}

#[cfg(unix)]
fn reader_loop(fifo: &std::path::Path, analysis: &SharedAnalysis) {
    // This open() will block until a writer (mpv via tee) connects
    let file = match std::fs::File::open(fifo) {
        Ok(f) => f,
        Err(_) => return,
    };

    // Minimal buffering — just enough for one chunk to reduce latency.
    // At 48kHz stereo s16le: 4 bytes per frame, 1024 frames = 4096 bytes.
    // Previous 16KB buffer could hold ~4 chunks, adding ~80ms of delay.
    const FRAMES: usize = FFT_SIZE;
    const BYTES_PER_FRAME: usize = 4; // 2 channels * 2 bytes (s16le)
    const CHUNK_SIZE: usize = FRAMES * BYTES_PER_FRAME;

    let mut reader = std::io::BufReader::with_capacity(CHUNK_SIZE, file);

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut mono_samples = vec![0.0f64; FRAMES];

    // Pre-allocate FFT work buffers to avoid per-frame allocation
    let mut fft_re = vec![0.0f64; FFT_SIZE];
    let mut fft_im = vec![0.0f64; FFT_SIZE];
    let mut magnitudes = vec![0.0f64; MAGNITUDE_COUNT];

    // Pre-compute Hann window coefficients
    let hann: Vec<f64> = (0..FFT_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (FFT_SIZE - 1) as f64).cos()))
        .collect();

    // Pre-compute logarithmic band edges (bin indices)
    let band_edges = compute_band_edges();

    loop {
        // Read a full chunk
        match reader.read_exact(&mut buf) {
            Ok(()) => {}
            Err(_) => {
                // FIFO closed or error — mpv stopped
                if let Ok(mut a) = analysis.lock() {
                    a.active = false;
                    a.rms = 0.0;
                    a.bands = [0.0; NUM_BANDS];
                }
                return;
            }
        }

        // Convert s16le stereo to mono f64 samples
        for i in 0..FRAMES {
            let offset = i * BYTES_PER_FRAME;
            let left = i16::from_le_bytes([buf[offset], buf[offset + 1]]) as f64;
            let right = i16::from_le_bytes([buf[offset + 2], buf[offset + 3]]) as f64;
            mono_samples[i] = (left + right) / 2.0 / 32768.0; // normalize to -1..1
        }

        // Compute overall RMS
        let rms = {
            let sum_sq: f64 = mono_samples.iter().map(|s| s * s).sum();
            (sum_sq / FRAMES as f64).sqrt()
        };

        // Apply Hann window and load into FFT real buffer, zero imaginary
        for i in 0..FFT_SIZE {
            fft_re[i] = mono_samples[i] * hann[i];
            fft_im[i] = 0.0;
        }

        // In-place radix-2 Cooley-Tukey FFT
        fft_in_place(&mut fft_re, &mut fft_im);

        // Compute magnitudes from first half (symmetric for real input)
        for i in 0..MAGNITUDE_COUNT {
            magnitudes[i] = (fft_re[i] * fft_re[i] + fft_im[i] * fft_im[i]).sqrt() / FFT_SIZE as f64;
        }

        // Group into logarithmically-spaced bands
        let band_energies = group_into_bands(&magnitudes, &band_edges);

        // Update shared state
        if let Ok(mut a) = analysis.lock() {
            a.active = true;
            a.rms = rms;
            a.bands = band_energies;
        }
    }
}

// ── FFT ─────────────────────────────────────────────────────────────

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

// ── Band grouping ───────────────────────────────────────────────────

/// Pre-compute logarithmic band edges as bin indices.
/// Returns NUM_BANDS + 1 edge values.
fn compute_band_edges() -> Vec<usize> {
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

/// Clean up the FIFO file
#[cfg(unix)]
pub fn cleanup_fifo(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
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