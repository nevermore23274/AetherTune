//! Windows-only WASAPI loopback audio capture.
//!
//! Captures whatever is playing through the default output device and
//! feeds it through the same FFT → band grouping → SeqLock pipeline
//! that the Unix parec/FIFO path uses. The visualizer doesn't need to
//! know which backend provided the data.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::audio::fft::{self, FFT_SIZE, MAGNITUDE_COUNT};
use crate::audio::pipe::{AudioAnalysis, SharedAnalysis, NUM_BANDS};

/// Spawn a background thread that captures system audio via WASAPI loopback.
///
/// The thread runs until `stop` is set to true. On any error (no audio
/// device, COM failure, etc.) the thread exits silently and the visualizer
/// falls back to simulated mode automatically.
pub fn spawn_capture_thread(
    analysis: SharedAnalysis,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        // Errors are non-fatal — the visualizer just stays in simulated mode
        let _ = capture_loop(&analysis, &stop);
        analysis.write(AudioAnalysis::new());
    })
}

fn capture_loop(
    analysis: &SharedAnalysis,
    stop: &AtomicBool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize COM on this thread (multi-threaded apartment)
    wasapi::initialize_mta()?;

    // Get the default output (render) device — capturing from it gives us
    // loopback audio (whatever is playing through the speakers)
    let enumerator = wasapi::DeviceEnumerator::new()?;
    let device = enumerator.get_default_device(&wasapi::Direction::Render)?;
    let mut audio_client = device.get_iaudioclient()?;

    // Query the device's native mix format — loopback capture delivers
    // audio in this format regardless of what we request
    let mix_format = audio_client.get_mixformat()?;
    let channels = mix_format.get_nchannels() as usize;
    let bits_per_sample = mix_format.get_bitspersample() as usize;
    let block_align = mix_format.get_blockalign() as usize;
    let bytes_per_sample = bits_per_sample / 8;

    // Initialize for loopback capture (shared mode, event-driven)
    // Direction::Capture on a Render device = loopback
    let (def_period, _min_period) = audio_client.get_device_period()?;
    audio_client.initialize_client(
        &mix_format,
        &wasapi::Direction::Capture,
        &wasapi::StreamMode::EventsShared {
            autoconvert: true,
            buffer_duration_hns: def_period,
        },
    )?;

    let h_event = audio_client.set_get_eventhandle()?;
    let capture_client = audio_client.get_audiocaptureclient()?;
    audio_client.start_stream()?;

    // ── FFT processing state (same pipeline as the Unix reader_loop) ──

    // Sliding window: we accumulate samples and run FFT every half-window,
    // giving ~94 updates/sec at 48kHz (same as the FIFO path).
    let half = FFT_SIZE / 2;
    let mut mono_samples = vec![0.0f64; FFT_SIZE];
    let mut sample_pos = half; // start half-full for sliding window

    // Pre-allocated FFT work buffers
    let mut fft_re = vec![0.0f64; FFT_SIZE];
    let mut fft_im = vec![0.0f64; FFT_SIZE];
    let mut magnitudes = vec![0.0f64; MAGNITUDE_COUNT];

    // Pre-compute Hann window and band edges
    let hann: Vec<f64> = (0..FFT_SIZE)
        .map(|i| {
            0.5 * (1.0
                - (2.0 * std::f64::consts::PI * i as f64 / (FFT_SIZE - 1) as f64).cos())
        })
        .collect();
    let band_edges = fft::compute_band_edges();
    let mut fft_count: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }

        // Wait for audio data (100ms timeout so we can check the stop flag)
        if h_event.wait_for_event(100_000).is_err() {
            continue;
        }

        // Drain all available packets (there may be more than one after an event)
        loop {
            let packet_size = match capture_client.get_next_packet_size() {
                Ok(Some(n)) if n > 0 => n as usize,
                _ => break,
            };

            let data = match capture_client.read_from_device(packet_size) {
                Ok(d) => d,
                Err(_) => break,
            };

            // Convert each frame to mono f64
            for frame in 0..packet_size {
                let offset = frame * block_align;

                let mono = if bits_per_sample == 32 {
                    // f32 samples (standard on modern Windows)
                    let mut sum = 0.0f64;
                    for ch in 0..channels {
                        let o = offset + ch * bytes_per_sample;
                        if o + 4 <= data.len() {
                            let s = f32::from_le_bytes([
                                data[o],
                                data[o + 1],
                                data[o + 2],
                                data[o + 3],
                            ]);
                            sum += s as f64;
                        }
                    }
                    sum / channels as f64
                } else if bits_per_sample == 16 {
                    // s16le samples (older devices)
                    let mut sum = 0.0f64;
                    for ch in 0..channels {
                        let o = offset + ch * bytes_per_sample;
                        if o + 2 <= data.len() {
                            let s = i16::from_le_bytes([data[o], data[o + 1]]);
                            sum += s as f64 / 32768.0;
                        }
                    }
                    sum / channels as f64
                } else {
                    0.0
                };

                mono_samples[sample_pos] = mono;
                sample_pos += 1;

                // When the window is full, run FFT and slide
                if sample_pos >= FFT_SIZE {
                    let rms = {
                        let sum_sq: f64 = mono_samples.iter().map(|s| s * s).sum();
                        (sum_sq / FFT_SIZE as f64).sqrt()
                    };

                    for i in 0..FFT_SIZE {
                        fft_re[i] = mono_samples[i] * hann[i];
                        fft_im[i] = 0.0;
                    }

                    fft::fft_in_place(&mut fft_re, &mut fft_im);

                    for i in 0..MAGNITUDE_COUNT {
                        magnitudes[i] = (fft_re[i] * fft_re[i] + fft_im[i] * fft_im[i]).sqrt()
                            / FFT_SIZE as f64;
                    }

                    let band_energies = fft::group_into_bands(&magnitudes, &band_edges);

                    fft_count += 1;
                    analysis.write(AudioAnalysis {
                        active: true,
                        rms,
                        bands: band_energies,
                        fft_count,
                    });

                    // Slide: keep second half, discard first
                    mono_samples.copy_within(half.., 0);
                    sample_pos = half;
                }
            }
        }
    }

    audio_client.stop_stream()?;
    Ok(())
}