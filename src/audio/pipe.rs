#[cfg(unix)]
use std::io::Read;
#[cfg(unix)]
use std::path::PathBuf;
use std::sync::Arc;

use super::seqlock::SeqLock;
use super::fft::{self, FFT_SIZE, MAGNITUDE_COUNT};

/// Number of frequency bands we compute for the visualizer
pub const NUM_BANDS: usize = 16;

/// Shared state between the reader thread and the main thread.
/// All fields are Copy — this is important for the SeqLock's
/// bytewise snapshot to be safe and meaningful.
#[derive(Clone, Copy)]
pub struct AudioAnalysis {
    /// Per-band energy levels, 0.0..1.0
    pub bands: [f64; NUM_BANDS],
    /// Overall RMS level, 0.0..1.0
    pub rms: f64,
    /// Whether the reader is actively receiving data
    pub active: bool,
    /// Total number of FFT updates since the reader started
    pub fft_count: u64,
}

impl AudioAnalysis {
    pub fn new() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            rms: 0.0,
            active: false,
            fft_count: 0,
        }
    }
}

pub type SharedAnalysis = Arc<SeqLock<AudioAnalysis>>;

/// Create the shared analysis state
pub fn new_shared_analysis() -> SharedAnalysis {
    Arc::new(SeqLock::new(AudioAnalysis::new()))
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

    // Sliding window approach: read STEP_FRAMES new samples at a time,
    // shift the sample buffer, and run FFT on the full FFT_SIZE window.
    // This doubles the FFT update rate without reducing frequency resolution.
    //
    // At 48kHz stereo s16le:
    //   FFT_SIZE  = 1024 samples = 4096 bytes = ~21.3ms (full window)
    //   STEP      =  512 samples = 2048 bytes = ~10.7ms (read chunk)
    //   FFT rate  = 48000 / 512  = ~94 updates/sec (vs ~47 with full reads)
    const STEP_FRAMES: usize = FFT_SIZE / 2;
    const BYTES_PER_FRAME: usize = 4; // 2 channels * 2 bytes (s16le)
    const STEP_BYTES: usize = STEP_FRAMES * BYTES_PER_FRAME;

    let mut reader = std::io::BufReader::with_capacity(STEP_BYTES, file);

    let mut read_buf = vec![0u8; STEP_BYTES];
    let mut mono_samples = vec![0.0f64; FFT_SIZE];

    // Pre-allocate FFT work buffers to avoid per-frame allocation
    let mut fft_re = vec![0.0f64; FFT_SIZE];
    let mut fft_im = vec![0.0f64; FFT_SIZE];
    let mut magnitudes = vec![0.0f64; MAGNITUDE_COUNT];

    // Pre-compute Hann window coefficients
    let hann: Vec<f64> = (0..FFT_SIZE)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (FFT_SIZE - 1) as f64).cos()))
        .collect();

    // Pre-compute logarithmic band edges (bin indices)
    let band_edges = fft::compute_band_edges();

    // Track FFT count locally — we can't read-modify-write through the seqlock
    let mut fft_count: u64 = 0;

    loop {
        // Read a half-window chunk (512 samples = 2048 bytes)
        match reader.read_exact(&mut read_buf) {
            Ok(()) => {}
            Err(_) => {
                // FIFO closed or error — mpv stopped
                analysis.write(AudioAnalysis {
                    active: false,
                    rms: 0.0,
                    bands: [0.0; NUM_BANDS],
                    fft_count,
                });
                return;
            }
        }

        // Shift the sample buffer left by STEP_FRAMES (discard oldest half)
        mono_samples.copy_within(STEP_FRAMES.., 0);

        // Convert new s16le stereo samples to mono f64 and fill the second half
        for i in 0..STEP_FRAMES {
            let offset = i * BYTES_PER_FRAME;
            let left = i16::from_le_bytes([read_buf[offset], read_buf[offset + 1]]) as f64;
            let right = i16::from_le_bytes([read_buf[offset + 2], read_buf[offset + 3]]) as f64;
            mono_samples[FFT_SIZE - STEP_FRAMES + i] = (left + right) / 2.0 / 32768.0;
        }

        // Compute overall RMS on the full window
        let rms = {
            let sum_sq: f64 = mono_samples.iter().map(|s| s * s).sum();
            (sum_sq / FFT_SIZE as f64).sqrt()
        };

        // Apply Hann window and load into FFT real buffer, zero imaginary
        for i in 0..FFT_SIZE {
            fft_re[i] = mono_samples[i] * hann[i];
            fft_im[i] = 0.0;
        }

        // In-place radix-2 Cooley-Tukey FFT
        fft::fft_in_place(&mut fft_re, &mut fft_im);

        // Compute magnitudes from first half (symmetric for real input)
        for i in 0..MAGNITUDE_COUNT {
            magnitudes[i] = (fft_re[i] * fft_re[i] + fft_im[i] * fft_im[i]).sqrt() / FFT_SIZE as f64;
        }

        // Group into logarithmically-spaced bands
        let band_energies = fft::group_into_bands(&magnitudes, &band_edges);

        // Update shared state
        fft_count += 1;
        analysis.write(AudioAnalysis {
            active: true,
            rms,
            bands: band_energies,
            fft_count,
        });
    }
}

/// Clean up the FIFO file
#[cfg(unix)]
pub fn cleanup_fifo(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}