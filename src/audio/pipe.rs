#[cfg(unix)]
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Number of frequency bands we compute for the visualizer
pub const NUM_BANDS: usize = 16;

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

    let mut reader = std::io::BufReader::with_capacity(16384, file);

    // We read in chunks that give us a window for analysis
    // At 48kHz stereo s16le: 4 bytes per frame, ~48000 frames/sec
    // A 2048-frame window = ~42ms of audio = 8192 bytes
    // We'll use 1024 frames for the DFT (mono-mixed)
    const FRAMES: usize = 1024;
    const BYTES_PER_FRAME: usize = 4; // 2 channels * 2 bytes (s16le)
    const CHUNK_SIZE: usize = FRAMES * BYTES_PER_FRAME;

    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut mono_samples = vec![0.0f64; FRAMES];

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

        // Compute frequency band energies using a simple DFT approach
        // We compute magnitude for specific frequency bins and group them
        // into NUM_BANDS (24) logarithmically spaced bands
        //
        // Frequency resolution: 48000 / 1024 ≈ 46.875 Hz per bin
        // Bin k corresponds to frequency k * 48000 / 1024
        // Useful range: bin 1 (~47Hz) to bin 512 (~24kHz)
        //
        // We use logarithmic spacing for the bands:
        // Band 0: ~47-100Hz (sub-bass)
        // Band 23: ~12-24kHz (air)
        let band_energies = compute_band_energies(&mono_samples, FRAMES);

        // Update shared state
        if let Ok(mut a) = analysis.lock() {
            a.active = true;
            a.rms = rms;
            a.bands = band_energies;
        }
    }
}

/// Compute energy in NUM_BANDS logarithmically-spaced frequency bands
/// using a partial DFT (only compute bins we need).
pub fn compute_band_energies(samples: &[f64], n: usize) -> [f64; NUM_BANDS] {
    let mut energies = [0.0f64; NUM_BANDS];

    // Define logarithmically spaced band edges in Hz
    // From ~50Hz to ~10kHz (CAVA recommended range for music visualization)
    let min_freq: f64 = 50.0;
    let max_freq: f64 = 10000.0;
    let sample_rate: f64 = 48000.0;
    let freq_resolution = sample_rate / n as f64; // ~46.875 Hz

    // Compute band edges in bin numbers
    let mut band_edges = Vec::with_capacity(NUM_BANDS + 1);
    for i in 0..=NUM_BANDS {
        let t = i as f64 / NUM_BANDS as f64;
        let freq = min_freq * (max_freq / min_freq).powf(t);
        let bin = (freq / freq_resolution).round() as usize;
        band_edges.push(bin.max(1).min(n / 2));
    }

    // For each band, compute the average magnitude of bins in that range
    // We use a real DFT: X[k] = sum(x[n] * e^(-j*2*pi*k*n/N))
    // magnitude = sqrt(re^2 + im^2)
    //
    // Optimization: we only compute the bins we actually need
    let two_pi_over_n = 2.0 * std::f64::consts::PI / n as f64;

    // Pre-apply a Hann window to reduce spectral leakage
    let windowed: Vec<f64> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let w = 0.5 * (1.0 - (two_pi_over_n * i as f64).cos());
            s * w
        })
        .collect();

    for band in 0..NUM_BANDS {
        let bin_start = band_edges[band];
        let bin_end = band_edges[band + 1];

        if bin_start >= bin_end {
            // Degenerate band — just compute one bin
            let k = bin_start;
            let (re, im) = dft_bin(&windowed, k, n, two_pi_over_n);
            let mag = (re * re + im * im).sqrt() / n as f64;
            energies[band] = mag;
            continue;
        }

        // For efficiency, don't compute every single bin in wide bands.
        // Sample up to 8 bins evenly across the range.
        let num_bins = bin_end - bin_start;
        let step = if num_bins > 8 { num_bins / 8 } else { 1 };
        let mut total_mag = 0.0;
        let mut count = 0;

        let mut k = bin_start;
        while k < bin_end {
            let (re, im) = dft_bin(&windowed, k, n, two_pi_over_n);
            let mag = (re * re + im * im).sqrt() / n as f64;
            total_mag += mag;
            count += 1;
            k += step;
        }

        energies[band] = if count > 0 {
            total_mag / count as f64
        } else {
            0.0
        };
    }

    // Normalize: find max energy and scale so the loudest band is ~1.0
    // Apply perceptual weighting — boost higher bands more aggressively
    // since energy naturally drops off with frequency in most music.
    // With 16 bands spanning 50Hz-10kHz, the upper bands need more help.
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

/// Compute a single DFT bin
#[inline]
pub fn dft_bin(samples: &[f64], k: usize, _n: usize, two_pi_over_n: f64) -> (f64, f64) {
    let mut re = 0.0;
    let mut im = 0.0;
    let w = two_pi_over_n * k as f64;

    for (i, &sample) in samples.iter().enumerate() {
        let angle = w * i as f64;
        re += sample * angle.cos();
        im -= sample * angle.sin();
    }

    (re, im)
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
    fn test_dft_silence() {
        let samples = vec![0.0f64; 1024];
        let energies = compute_band_energies(&samples, 1024);
        assert!(
            energies.iter().all(|&e| e < 0.001),
            "Silent input should produce near-zero energy in all bands"
        );
    }

    #[test]
    fn test_dft_single_tone_440hz() {
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
    fn test_dft_high_tone_vs_low_tone() {
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
    fn test_dft_bin_dc() {
        // Bin 0 (DC) of a constant signal should have high magnitude
        let samples = vec![1.0; 1024];
        let two_pi_over_n = 2.0 * PI / 1024.0;
        let (re, im) = dft_bin(&samples, 0, 1024, two_pi_over_n);
        let mag = (re * re + im * im).sqrt();
        assert!(
            mag > 100.0,
            "DC bin of constant signal should have high magnitude, got {}",
            mag
        );
    }

    #[test]
    fn test_band_count() {
        let samples = sine_wave(1000.0, 48000.0, 1024);
        let energies = compute_band_energies(&samples, 1024);
        assert_eq!(energies.len(), NUM_BANDS);
    }
}