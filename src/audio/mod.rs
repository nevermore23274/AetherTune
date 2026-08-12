pub mod seqlock;
pub mod fft;
pub mod pipe;
pub mod player;
pub mod visualizer;

#[cfg(windows)]
pub mod wasapi_capture;
#[cfg(windows)]
pub mod jobobject;

#[cfg(target_os = "macos")]
pub mod coreaudio_capture;