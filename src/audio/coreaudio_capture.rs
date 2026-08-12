//! macOS-only real-time system audio capture via a Core Audio process tap
//! (macOS 14.4+ — see `AudioHardwareCreateProcessTap` in Apple's docs).
//!
//! Builds an unmuted, system-wide tap (mixes every process's output down to
//! stereo without silencing it), aggregates that tap with the current
//! default output device purely to anchor its clock, and reads PCM through
//! a plain `AudioDeviceIOProc` callback. No external driver, no user setup
//! beyond the one-time system audio-capture permission prompt.
//!
//! Feeds the same FFT -> band-grouping -> `SeqLock` pipeline used by the
//! Linux FIFO reader (`pipe.rs`) and the Windows WASAPI capture
//! (`wasapi_capture.rs`), so the visualizer doesn't need to know which
//! backend produced the data.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use objc2::AnyThread;
use objc2_core_audio::{
    self as ca, AudioDeviceCreateIOProcID, AudioDeviceDestroyIOProcID, AudioDeviceIOProcID,
    AudioDeviceStart, AudioDeviceStop, AudioHardwareCreateAggregateDevice,
    AudioHardwareCreateProcessTap, AudioHardwareDestroyAggregateDevice,
    AudioHardwareDestroyProcessTap, AudioObjectGetPropertyData, AudioObjectID,
    AudioObjectPropertyAddress, CATapDescription, CATapMuteBehavior,
};
use objc2_core_audio_types::{AudioBufferList, AudioTimeStamp};
use objc2_core_foundation::{CFArray, CFBoolean, CFDictionary, CFRetained, CFString, CFType};
use objc2_foundation::{NSArray, NSNumber, NSString};

use crate::audio::fft::{self, FFT_SIZE, MAGNITUDE_COUNT};
use crate::audio::pipe::{AudioAnalysis, SharedAnalysis};

/// Core Audio process taps require macOS 14.4 (Sonoma) or newer.
const MIN_MACOS_VERSION: (u32, u32) = (14, 4);

/// True if the running system is new enough to support process taps.
///
/// Checked by shelling out to `sw_vers` rather than calling into the tap
/// APIs speculatively — that way we never reference a symbol that doesn't
/// exist on older systems, which would otherwise risk a hard crash the
/// first time it's touched.
pub fn taps_supported() -> bool {
    let Ok(output) = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version.trim().split('.').map(|p| p.parse::<u32>().unwrap_or(0));
    let major = parts.next().unwrap_or(0);
    let minor = parts.next().unwrap_or(0);
    (major, minor) >= MIN_MACOS_VERSION
}

/// Spawn a background thread that sets up a Core Audio process tap and
/// blocks until `stop` is set. The FFT/band pipeline runs inside the
/// `AudioDeviceIOProc` callback registered on the aggregate device (Core
/// Audio's own thread) — this thread only owns setup and teardown.
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

/// State shared with the real-time IOProc callback via a raw pointer. Owned
/// by `capture_loop` for its whole lifetime; the callback only ever
/// dereferences it while the IOProc is registered and running.
struct CaptureState {
    analysis: SharedAnalysis,
    mono_samples: Vec<f64>,
    sample_pos: usize,
    fft_re: Vec<f64>,
    fft_im: Vec<f64>,
    magnitudes: Vec<f64>,
    hann: Vec<f64>,
    band_edges: Vec<usize>,
    fft_count: u64,
}

impl CaptureState {
    fn new(analysis: SharedAnalysis) -> Self {
        let half = FFT_SIZE / 2;
        let hann: Vec<f64> = (0..FFT_SIZE)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f64::consts::PI * i as f64 / (FFT_SIZE - 1) as f64).cos())
            })
            .collect();

        Self {
            analysis,
            mono_samples: vec![0.0; FFT_SIZE],
            sample_pos: half, // start half-full for sliding window, same as WASAPI path
            fft_re: vec![0.0; FFT_SIZE],
            fft_im: vec![0.0; FFT_SIZE],
            magnitudes: vec![0.0; MAGNITUDE_COUNT],
            hann,
            band_edges: fft::compute_band_edges(),
            fft_count: 0,
        }
    }

    /// Consume one mono sample, running the FFT and publishing to the
    /// shared analysis state whenever the sliding window fills up.
    fn push_sample(&mut self, mono: f64) {
        let half = FFT_SIZE / 2;
        self.mono_samples[self.sample_pos] = mono;
        self.sample_pos += 1;

        if self.sample_pos < FFT_SIZE {
            return;
        }

        let rms = {
            let sum_sq: f64 = self.mono_samples.iter().map(|s| s * s).sum();
            (sum_sq / FFT_SIZE as f64).sqrt()
        };

        for i in 0..FFT_SIZE {
            self.fft_re[i] = self.mono_samples[i] * self.hann[i];
            self.fft_im[i] = 0.0;
        }

        fft::fft_in_place(&mut self.fft_re, &mut self.fft_im);

        for i in 0..MAGNITUDE_COUNT {
            self.magnitudes[i] = (self.fft_re[i] * self.fft_re[i]
                + self.fft_im[i] * self.fft_im[i])
                .sqrt()
                / FFT_SIZE as f64;
        }

        let band_energies = fft::group_into_bands(&self.magnitudes, &self.band_edges);

        self.fft_count += 1;
        self.analysis.write(AudioAnalysis {
            active: true,
            rms,
            bands: band_energies,
            fft_count: self.fft_count,
        });

        // Slide: keep second half, discard first
        self.mono_samples.copy_within(half.., 0);
        self.sample_pos = half;
    }
}

fn capture_loop(analysis: &SharedAnalysis, stop: &AtomicBool) -> Result<(), String> {
    if !taps_supported() {
        return Err("Core Audio process taps require macOS 14.4+".into());
    }

    // ── 1. Build an unmuted, private, system-wide tap ──
    let exclude = NSArray::<NSNumber>::from_slice(&[]);
    let tap_description = unsafe {
        CATapDescription::initStereoGlobalTapButExcludeProcesses(
            CATapDescription::alloc(),
            &exclude,
        )
    };
    unsafe { tap_description.setMuteBehavior(CATapMuteBehavior::Unmuted) };
    unsafe { tap_description.setPrivate(true) };
    let tap_name = NSString::from_str("AetherTune System Audio Tap");
    unsafe { tap_description.setName(&tap_name) };

    let mut tap_id: AudioObjectID = ca::kAudioObjectUnknown;
    let status = unsafe { AudioHardwareCreateProcessTap(Some(&tap_description), &mut tap_id) };
    if status != 0 {
        return Err(format!("AudioHardwareCreateProcessTap failed: {status}"));
    }
    // Tears the tap down on every exit path below, success or error.
    let _tap_guard = TapGuard(tap_id);

    let tap_uid = unsafe { tap_description.UUID() }.UUIDString().to_string();

    // ── 2. Resolve the current default output device's UID. This only
    //    anchors the aggregate's clock — it is NOT added as a sub-device,
    //    so audio keeps playing through it completely normally (the tap
    //    itself, being unmuted, is what lets sound still reach the
    //    speakers). ──
    let output_device_id = get_default_output_device()?;
    let output_uid = get_device_uid(output_device_id)?;

    // ── 3. Build and create the aggregate device that exposes the tap as
    //    a readable input stream ──
    let aggregate_uid = format!("com.aethertune.tap.{}", std::process::id());
    let aggregate_dict = build_aggregate_device_dict(&aggregate_uid, &output_uid, &tap_uid);

    let mut aggregate_id: AudioObjectID = ca::kAudioObjectUnknown;
    let status = unsafe {
        AudioHardwareCreateAggregateDevice(aggregate_dict.as_ref(), (&mut aggregate_id).into())
    };
    if status != 0 {
        return Err(format!("AudioHardwareCreateAggregateDevice failed: {status}"));
    }
    let _aggregate_guard = AggregateGuard(aggregate_id);

    // ── 4. Register the IOProc and start pulling frames ──
    // `state` is heap-allocated and handed to Core Audio as an opaque raw
    // pointer via `Box::into_raw` — Rust's normal ownership tracking can't
    // see the callback holding a reference to it, so we manage its lifetime
    // by hand: it must not be freed until AFTER the IOProc has been fully
    // stopped and destroyed (guaranteeing no callback can still be touching
    // it), which is why that happens explicitly below rather than through
    // an ordinary RAII guard.
    let state_ptr: *mut CaptureState = Box::into_raw(Box::new(CaptureState::new(analysis.clone())));

    let mut ioproc_id: AudioDeviceIOProcID = None;
    let status = unsafe {
        AudioDeviceCreateIOProcID(
            aggregate_id,
            Some(io_proc),
            state_ptr as *mut c_void,
            (&mut ioproc_id).into(),
        )
    };
    if status != 0 {
        // No IOProc was ever registered, so it's safe to free immediately.
        drop(unsafe { Box::from_raw(state_ptr) });
        return Err(format!("AudioDeviceCreateIOProcID failed: {status}"));
    }
    // Guards early-return paths between here and the manual teardown below
    // (e.g. if AudioDeviceStart fails). `teardown()` tracks whether it has
    // already run, so calling it explicitly on the happy path below makes
    // the later automatic `Drop` a no-op rather than a double-teardown.
    let mut ioproc_guard = IoProcGuard::armed(aggregate_id, ioproc_id, state_ptr);

    let status = unsafe { AudioDeviceStart(aggregate_id, ioproc_id) };
    if status != 0 {
        return Err(format!("AudioDeviceStart failed: {status}"));
    }

    // All the real work happens in `io_proc` on Core Audio's own thread.
    // We just wait here until told to stop.
    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }

    // Happy-path teardown, in order: stop + destroy the IOProc (after this,
    // Core Audio guarantees no further callback), THEN free `state`. This
    // marks the guard as torn down, so its `Drop` at function exit is a
    // no-op. `_aggregate_guard` and `_tap_guard` still run normally after
    // this function returns.
    ioproc_guard.teardown();

    Ok(())
}

/// The real-time audio callback. Reads whatever the tap delivered this
/// cycle, converts it to mono f64, and feeds it through `CaptureState`.
///
/// # Safety
/// Called by Core Audio with `in_client_data` set to the `*mut CaptureState`
/// passed to `AudioDeviceCreateIOProcID`. That pointer is guaranteed valid
/// for as long as the IOProc is registered (see `IoProcGuard`).
unsafe extern "C-unwind" fn io_proc(
    _in_device: AudioObjectID,
    _in_now: NonNull<AudioTimeStamp>,
    in_input_data: NonNull<AudioBufferList>,
    _in_input_time: NonNull<AudioTimeStamp>,
    _out_output_data: NonNull<AudioBufferList>,
    _in_output_time: NonNull<AudioTimeStamp>,
    in_client_data: *mut c_void,
) -> i32 {
    if in_client_data.is_null() {
        return 0;
    }
    // SAFETY: valid for the IOProc's registered lifetime, see doc comment.
    let state = unsafe { &mut *(in_client_data as *mut CaptureState) };

    // SAFETY: Core Audio guarantees `in_input_data` points at a valid
    // AudioBufferList for the duration of this call. The tap's format is a
    // stereo mixdown, so we expect a single interleaved buffer.
    let buffer_list = unsafe { in_input_data.as_ref() };
    if buffer_list.mNumberBuffers == 0 {
        return 0;
    }
    let buffer = &buffer_list.mBuffers[0];
    if buffer.mData.is_null() || buffer.mNumberChannels == 0 {
        return 0;
    }

    let channels = buffer.mNumberChannels as usize;
    let sample_count = (buffer.mDataByteSize as usize) / std::mem::size_of::<f32>();
    let frame_count = sample_count / channels;

    // SAFETY: `mData` + `mDataByteSize` describe a valid Float32 buffer of
    // `sample_count` samples for the duration of this call (Core Audio
    // process taps always deliver Float32 — see `kAudioTapPropertyFormat`).
    let samples = unsafe {
        std::slice::from_raw_parts(buffer.mData as *const f32, sample_count)
    };

    for frame in 0..frame_count {
        let mut sum = 0.0f64;
        for ch in 0..channels {
            sum += samples[frame * channels + ch] as f64;
        }
        state.push_sample(sum / channels as f64);
    }

    0
}

fn get_default_output_device() -> Result<AudioObjectID, String> {
    let address = AudioObjectPropertyAddress {
        mSelector: ca::kAudioHardwarePropertyDefaultOutputDevice,
        mScope: ca::kAudioObjectPropertyScopeGlobal,
        mElement: ca::kAudioObjectPropertyElementMain,
    };
    let mut device_id: AudioObjectID = ca::kAudioObjectUnknown;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    let out = NonNull::new(&mut device_id as *mut AudioObjectID as *mut c_void)
        .ok_or("null output pointer")?;
    let status = unsafe {
        AudioObjectGetPropertyData(
            ca::kAudioObjectSystemObject as AudioObjectID,
            (&address).into(),
            0,
            std::ptr::null(),
            (&mut size).into(),
            out,
        )
    };
    if status != 0 {
        return Err(format!("failed to read default output device: {status}"));
    }
    Ok(device_id)
}

fn get_device_uid(device_id: AudioObjectID) -> Result<String, String> {
    let address = AudioObjectPropertyAddress {
        mSelector: ca::kAudioDevicePropertyDeviceUID,
        mScope: ca::kAudioObjectPropertyScopeGlobal,
        mElement: ca::kAudioObjectPropertyElementMain,
    };
    let mut cf_string_ptr: *const CFString = std::ptr::null();
    let mut size = std::mem::size_of::<*const CFString>() as u32;
    let out = NonNull::new(&mut cf_string_ptr as *mut *const CFString as *mut c_void)
        .ok_or("null output pointer")?;
    let status = unsafe {
        AudioObjectGetPropertyData(device_id, (&address).into(), 0, std::ptr::null(), (&mut size).into(), out)
    };
    if status != 0 {
        return Err(format!("failed to read device UID: {status}"));
    }
    let ptr = NonNull::new(cf_string_ptr as *mut CFString)
        .ok_or("device UID property returned a null string")?;
    // AudioObjectGetPropertyData hands back a reference we don't already
    // own; retain it so we hold a safe, independently-owned copy rather
    // than guessing at the Get-vs-Copy ownership convention.
    let owned: CFRetained<CFString> = unsafe { CFRetained::retain(ptr) };
    Ok(owned.to_string())
}

/// Convert one of Core Audio's `&'static CStr` dictionary-key constants
/// into a `CFString` usable as a `CFDictionary` key.
fn cfstr(s: &std::ffi::CStr) -> CFRetained<CFString> {
    CFString::from_str(s.to_str().expect("CoreAudio key constant is valid UTF-8"))
}

/// Build the dictionary passed to `AudioHardwareCreateAggregateDevice`:
/// a private aggregate device anchored on the real output device, with our
/// tap attached as its sole readable stream.
fn build_aggregate_device_dict(
    aggregate_uid: &str,
    output_device_uid: &str,
    tap_uid: &str,
) -> CFRetained<CFDictionary<CFString, CFType>> {
    let name = CFString::from_str("AetherTune Visualizer Tap");
    let uid = CFString::from_str(aggregate_uid);
    let main_sub_device = CFString::from_str(output_device_uid);

    let sub_tap_uid_key = cfstr(ca::kAudioSubTapUIDKey);
    let sub_tap_uid_value = CFString::from_str(tap_uid);
    let sub_tap_dict: CFRetained<CFDictionary<CFString, CFType>> = CFDictionary::from_slices(
        &[&*sub_tap_uid_key],
        &[sub_tap_uid_value.as_ref()],
    );
    let tap_list = CFArray::<CFType>::from_objects(&[sub_tap_dict.as_ref()]);

    let name_key = cfstr(ca::kAudioAggregateDeviceNameKey);
    let uid_key = cfstr(ca::kAudioAggregateDeviceUIDKey);
    let main_sub_device_key = cfstr(ca::kAudioAggregateDeviceMainSubDeviceKey);
    let is_private_key = cfstr(ca::kAudioAggregateDeviceIsPrivateKey);
    let tap_auto_start_key = cfstr(ca::kAudioAggregateDeviceTapAutoStartKey);
    let tap_list_key = cfstr(ca::kAudioAggregateDeviceTapListKey);

    CFDictionary::from_slices(
        &[
            &*name_key,
            &*uid_key,
            &*main_sub_device_key,
            &*is_private_key,
            &*tap_auto_start_key,
            &*tap_list_key,
        ],
        &[
            name.as_ref(),
            uid.as_ref(),
            main_sub_device.as_ref(),
            CFBoolean::new(true).as_ref(),
            CFBoolean::new(true).as_ref(),
            tap_list.as_ref(),
        ],
    )
}

struct TapGuard(AudioObjectID);
impl Drop for TapGuard {
    fn drop(&mut self) {
        unsafe { AudioHardwareDestroyProcessTap(self.0) };
    }
}

struct AggregateGuard(AudioObjectID);
impl Drop for AggregateGuard {
    fn drop(&mut self) {
        unsafe { AudioHardwareDestroyAggregateDevice(self.0) };
    }
}

/// Owns the registered IOProc and the `CaptureState` it points at. Tears
/// both down together, in the only safe order: stop/destroy the IOProc
/// *first* (so Core Audio guarantees no further callback), then free the
/// state. Runs automatically on drop for early-error paths; the happy path
/// calls `teardown()` explicitly once the wait loop exits, after which
/// `Drop` becomes a no-op.
struct IoProcGuard {
    device_id: AudioObjectID,
    ioproc_id: AudioDeviceIOProcID,
    state_ptr: *mut CaptureState,
    torn_down: bool,
}

impl IoProcGuard {
    fn armed(device_id: AudioObjectID, ioproc_id: AudioDeviceIOProcID, state_ptr: *mut CaptureState) -> Self {
        Self { device_id, ioproc_id, state_ptr, torn_down: false }
    }

    fn teardown(&mut self) {
        if self.torn_down {
            return;
        }
        unsafe {
            AudioDeviceStop(self.device_id, self.ioproc_id);
            AudioDeviceDestroyIOProcID(self.device_id, self.ioproc_id);
            drop(Box::from_raw(self.state_ptr));
        }
        self.torn_down = true;
    }
}

impl Drop for IoProcGuard {
    fn drop(&mut self) {
        self.teardown();
    }
}
