use std::time::{Duration, Instant};

/// Internal clock of proposer.
#[derive(Clone)]
pub(crate) struct InternalClock {
    time: Instant,
    elapsed: Duration,
}

impl InternalClock {
    pub(crate) fn new(initial_time: Instant) -> Self {
        Self {
            time: initial_time,
            elapsed: Duration::new(0, 0),
        }
    }

    pub(crate) fn update(&mut self, new_time: Instant) -> &mut Self {
        if self.time >= new_time {
            // just to guard against panic.
            return self;
        }
        self.elapsed += new_time.duration_since(self.time);
        self.time = new_time;
        self
    }

    /// Result of every call of this method MUST be passed to state machine
    /// `TezedgeState` through proposal, otherwise internal clock will mess up.
    #[inline]
    pub(crate) fn take_elapsed(&mut self) -> Duration {
        std::mem::replace(&mut self.elapsed, Duration::new(0, 0))
    }
}
