//! Thin re-export facade.
//!
//! The real implementations now live under `crate::core::*`.
//! This module exists so that all existing `use crate::app::{App, Overlay, ...}`
//! imports throughout the UI layer continue to work unchanged.

pub use crate::core::app::App;
pub use crate::core::perf::{FrameTiming, PerfStats, PerfSummary};
pub use crate::core::radio::FetchResult;
pub use crate::core::types::{ActivePanel, InputMode, NowPlaying, Overlay, QueryKind, SongLogEntry};