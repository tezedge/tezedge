use std::time::{Instant, Duration};
use crate::handlers::handler_messages::IncomingTransferMetrics;

#[derive(Default)]
struct LinRegStats {
    sum_x: f32,
    sum_y: f32,
    sum_xy: f32,
    sum_x_sqr: f32,
    sum_y_sqr: f32,
    sample_size: f32,
}

impl LinRegStats {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_observation(&mut self, x: &f32, y: &f32) {
        self.sum_x += *x;
        self.sum_y += *y;
        self.sum_xy += x * y;
        self.sum_x_sqr += x.powi(2);
        self.sum_y_sqr += y.powi(2);
        self.sample_size += 1f32;
    }

    fn calculate_prediction(&self, point: f32) -> f32 {
        let intercept = (self.sum_y * self.sum_x_sqr) - ((self.sum_x * self.sum_xy) / (self.sample_size * self.sum_x)) - self.sum_x.powi(2);
        let slope = (self.sample_size * self.sum_xy) - ((self.sum_x * self.sum_y) / self.sample_size * self.sum_x_sqr) - self.sum_x.powi(2);
        intercept + slope * point
    }
}

/// General statistics about incoming transfer
pub(crate) struct BootstrapMonitor {
    // total number of blocks
    pub level: usize,
    // already downloaded blocks
    downloaded_blocks: usize,
    // number of blocks downloaded per this session
    downloaded_per_session: usize,
    downloaded_per_snapshot: usize,
    session_start: Instant,
    last_snapshot: Instant,
    eta_prediction: LinRegStats,
}

impl BootstrapMonitor {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            level: 0,
            downloaded_blocks: 0,
            downloaded_per_session: 0,
            downloaded_per_snapshot: 0,
            session_start: now.clone(),
            last_snapshot: now,
            eta_prediction: LinRegStats::new(),
        }
    }

    #[inline]
    pub fn missing_blocks(&self) -> usize {
        self.level - self.downloaded_per_session
    }

    pub fn downloaded_blocks(&self) -> usize {
        self.downloaded_blocks
    }

    pub fn snapshot(&mut self) -> IncomingTransferMetrics {
        let snapshot_end = Instant::now();
        let snapshot_duration = snapshot_end - self.last_snapshot;
        let downloaded_during_snapshot = self.downloaded_per_snapshot;
        let current_bps = downloaded_during_snapshot as f32 / snapshot_duration.as_secs_f32();
        let _expected_eta = self.missing_blocks() as f32 / current_bps;

        self.downloaded_per_snapshot = 0;
        self.last_snapshot = Instant::now();
    }

    pub fn calculate_eta(&self) -> Option<Duration> {
        let prediction = self.eta_prediction.calculate_prediction(self.session_start.elapsed().as_secs_f32());
        if prediction.is_finite() {
            Some(Duration::from_secs_f32(prediction))
        } else {
            None
        }
    }
}