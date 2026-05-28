use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// A sequence lock for single-producer / single-consumer latest-value handoff.
///
/// The producer increments the sequence to an odd number before writing,
/// then increments again to even when done. The consumer reads the sequence,
/// copies the data, and checks the sequence again — if both reads match and
/// are even, the copy is consistent. Otherwise it retries.
///
/// This is lock-free on both sides: the producer never blocks, and the
/// consumer only spins for the duration of a write (~nanoseconds).
pub struct SeqLock<T: Copy> {
    /// Odd = write in progress, even = safe to read
    sequence: AtomicU64,
    data: UnsafeCell<T>,
}

// SAFETY: SeqLock's protocol guarantees the consumer never observes a torn
// write — it detects partial writes via the sequence counter and retries.
// The producer is the only writer, and increments the sequence atomically
// around each write. This is the standard seqlock invariant.
unsafe impl<T: Copy> Sync for SeqLock<T> {}
unsafe impl<T: Copy> Send for SeqLock<T> {}

impl<T: Copy> SeqLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            data: UnsafeCell::new(value),
        }
    }

    /// Producer: atomically publish a new value.
    /// Only one thread may call this (single-producer constraint).
    pub fn write(&self, value: T) {
        // Mark sequence odd (write in progress)
        self.sequence.fetch_add(1, Ordering::Release);

        // SAFETY: single producer guarantee — no concurrent writes.
        // Consumers detect this in-progress write via the odd sequence
        // and retry, so they never read torn data.
        unsafe { *self.data.get() = value; }

        // Mark sequence even (write complete)
        self.sequence.fetch_add(1, Ordering::Release);
    }

    /// Consumer: read the latest consistent snapshot.
    /// Spins briefly if a write is in progress (sub-microsecond).
    pub fn read(&self) -> T {
        loop {
            let s1 = self.sequence.load(Ordering::Acquire);
            if s1 & 1 != 0 {
                // Writer is mid-update — spin
                std::hint::spin_loop();
                continue;
            }

            // SAFETY: sequence is even, so no write is in progress.
            // We copy the full T, then verify the sequence hasn't changed.
            let snapshot = unsafe { *self.data.get() };

            let s2 = self.sequence.load(Ordering::Acquire);
            if s1 == s2 {
                return snapshot;
            }
            // Sequence changed — a write slipped in, retry
            std::hint::spin_loop();
        }
    }
}