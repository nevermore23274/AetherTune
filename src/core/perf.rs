/// Lightweight per-frame performance counters.
/// Tracks timing for each phase of the main loop so we can
/// see the cost of increasing the tick rate.
pub struct PerfStats {
    /// Ring buffer of recent frame timings
    samples: Vec<FrameTiming>,
    write_idx: usize,
    capacity: usize,
    /// Rolling CPU load history for sparkline (0.0..1.0 values)
    pub load_history: Vec<f64>,
    load_write_idx: usize,
    load_capacity: usize,
}

#[derive(Clone, Copy, Default)]
pub struct FrameTiming {
    pub draw_us: u64,
    /// Idle time spent in event::poll() waiting for input or timeout
    pub event_wait_us: u64,
    /// Actual work done handling key events after poll returns
    pub event_handle_us: u64,
    pub poll_us: u64,
    pub vis_us: u64,
    pub total_us: u64,
    /// Whether this frame included a tick (poll + vis ran)
    pub had_tick: bool,
}

impl FrameTiming {
    /// CPU work only — excludes the idle poll wait
    pub fn work_us(&self) -> u64 {
        self.draw_us + self.event_handle_us + self.poll_us + self.vis_us
    }
}

/// Summary stats with separate tick-only averages for poll/vis
pub struct PerfSummary {
    pub avg: FrameTiming,
    pub max: FrameTiming,
    /// Average poll_us computed only over frames that had a tick
    pub tick_avg_poll_us: u64,
    /// Average vis_us computed only over frames that had a tick
    pub tick_avg_vis_us: u64,
    /// Max poll_us from tick frames only
    pub tick_max_poll_us: u64,
    /// Max vis_us from tick frames only
    pub tick_max_vis_us: u64,
}

impl PerfStats {
    pub fn new() -> Self {
        let capacity = 120; // ~4 seconds at 30ms tick
        let load_capacity = 40; // sparkline width
        Self {
            samples: vec![FrameTiming::default(); capacity],
            write_idx: 0,
            capacity,
            load_history: vec![0.0; load_capacity],
            load_write_idx: 0,
            load_capacity,
        }
    }

    pub fn record(&mut self, timing: FrameTiming, tick_budget_us: u64) {
        self.samples[self.write_idx] = timing;
        self.write_idx = (self.write_idx + 1) % self.capacity;

        // Record load sample for sparkline
        let load = if tick_budget_us > 0 {
            (timing.work_us() as f64 / tick_budget_us as f64).min(1.0)
        } else {
            0.0
        };
        self.load_history[self.load_write_idx] = load;
        self.load_write_idx = (self.load_write_idx + 1) % self.load_capacity;
    }

    /// Returns comprehensive summary with tick-aware averaging
    pub fn summary(&self) -> PerfSummary {
        let mut avg = FrameTiming::default();
        let mut max = FrameTiming::default();
        let mut count = 0u64;

        // Separate counters for tick frames
        let mut tick_poll_sum = 0u64;
        let mut tick_vis_sum = 0u64;
        let mut tick_poll_max = 0u64;
        let mut tick_vis_max = 0u64;
        let mut tick_count = 0u64;

        // Only look at the most recent window for max (rolling window max)
        let window = self.capacity.min(60); // ~2 seconds of frames
        for i in 0..window {
            let idx = (self.write_idx + self.capacity - 1 - i) % self.capacity;
            let s = &self.samples[idx];
            if s.total_us == 0 {
                continue;
            }

            avg.draw_us += s.draw_us;
            avg.event_wait_us += s.event_wait_us;
            avg.event_handle_us += s.event_handle_us;
            avg.poll_us += s.poll_us;
            avg.vis_us += s.vis_us;
            avg.total_us += s.total_us;
            count += 1;

            max.draw_us = max.draw_us.max(s.draw_us);
            max.event_wait_us = max.event_wait_us.max(s.event_wait_us);
            max.event_handle_us = max.event_handle_us.max(s.event_handle_us);
            max.total_us = max.total_us.max(s.total_us);

            if s.had_tick {
                tick_poll_sum += s.poll_us;
                tick_vis_sum += s.vis_us;
                tick_poll_max = tick_poll_max.max(s.poll_us);
                tick_vis_max = tick_vis_max.max(s.vis_us);
                tick_count += 1;
            }
        }

        if count > 0 {
            avg.draw_us /= count;
            avg.event_wait_us /= count;
            avg.event_handle_us /= count;
            avg.poll_us /= count;
            avg.vis_us /= count;
            avg.total_us /= count;
        }

        // Compute max for work_us from per-frame work
        for i in 0..window {
            let idx = (self.write_idx + self.capacity - 1 - i) % self.capacity;
            let s = &self.samples[idx];
            if s.total_us > 0 {
                let w = s.work_us();
                let existing = max.draw_us.max(max.event_handle_us) + max.poll_us + max.vis_us;
                if w > existing {
                    // We track this through the individual maxes already
                }
            }
        }

        PerfSummary {
            avg,
            max,
            tick_avg_poll_us: if tick_count > 0 { tick_poll_sum / tick_count } else { 0 },
            tick_avg_vis_us: if tick_count > 0 { tick_vis_sum / tick_count } else { 0 },
            tick_max_poll_us: tick_poll_max,
            tick_max_vis_us: tick_vis_max,
        }
    }

    /// Get the load history ordered oldest-to-newest for sparkline rendering
    pub fn load_history_ordered(&self) -> Vec<f64> {
        let mut result = Vec::with_capacity(self.load_capacity);
        for i in 0..self.load_capacity {
            let idx = (self.load_write_idx + i) % self.load_capacity;
            result.push(self.load_history[idx]);
        }
        result
    }
}