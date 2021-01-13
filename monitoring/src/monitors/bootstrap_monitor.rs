// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crate::handlers::handler_messages::IncomingTransferMetrics;

/// General statistics about incoming transfer
pub(crate) struct BootstrapMonitor {
    // TODO: review
    // total number of blocks
    level: usize,
    // already downloaded blocks
    downloaded_blocks: usize,
    downloaded_headers: usize,
    // number of blocks downloaded per this session
    downloaded_per_session: usize,
    downloaded_per_snapshot: usize,
    headers_per_session: usize,
    headers_per_snapshot: usize,
    session_start: Instant,
    last_snapshot: Instant,
}

impl Default for BootstrapMonitor {
    fn default() -> Self {
        Self {
            level: 0,
            downloaded_blocks: 0,
            downloaded_headers: 0,
            downloaded_per_session: 0,
            downloaded_per_snapshot: 0,
            headers_per_session: 0,
            headers_per_snapshot: 0,
            session_start: Instant::now(),
            last_snapshot: Instant::now(),
        }
    }
}

impl BootstrapMonitor {
    #[inline]
    pub fn set_level(&mut self, level: usize) {
        // only set peer current head level for values higher than the current level
        self.level = if self.level > level {
            self.level
        } else {
            level
        }
    }

    #[inline]
    pub fn missing_blocks(&self) -> usize {
        if self.level >= self.downloaded_per_session {
            self.level - self.downloaded_per_session
        } else {
            0
        }
    }

    #[inline]
    pub fn increase_block_count(&mut self) {
        self.increase_block_count_by(1);
    }

    #[inline]
    pub fn increase_block_count_by(&mut self, count: usize) {
        self.downloaded_per_snapshot += count;
        self.downloaded_per_session += count;
        self.downloaded_blocks += count;
    }
    #[inline]
    pub fn increase_headers_count(&mut self) {
        self.increase_headers_count_by(1)
    }

    #[inline]
    pub fn increase_headers_count_by(&mut self, count: usize) {
        self.headers_per_session += count;
        self.headers_per_snapshot += count;
        self.downloaded_headers += count;
    }

    #[inline]
    pub fn average_download_rate(&self) -> f32 {
        self.downloaded_per_session as f32 / self.session_start.elapsed().as_secs_f32()
    }

    #[inline]
    pub fn average_header_download_rate(&self) -> f32 {
        self.headers_per_session as f32 / self.session_start.elapsed().as_secs_f32()
    }

    pub fn snapshot(&mut self) -> IncomingTransferMetrics {
        use std::f32;
        let snapshot_end = Instant::now();
        let snapshot_duration = snapshot_end - self.last_snapshot;

        let downloaded_blocks = self.downloaded_per_snapshot;
        let current_bps = downloaded_blocks as f32 / snapshot_duration.as_secs_f32();
        let current_eta = self.missing_blocks() as f32 / current_bps;
        let current_header_bps = self.headers_per_snapshot as f32 / snapshot_duration.as_secs_f32();

        self.downloaded_per_snapshot = 0;
        self.headers_per_snapshot = 0;
        self.last_snapshot = snapshot_end;

        IncomingTransferMetrics::new(
            current_eta,
            self.level,
            self.downloaded_blocks,
            current_bps,
            self.average_download_rate(),
            self.downloaded_headers,
            current_header_bps,
            self.average_header_download_rate(),
        )
    }
}
