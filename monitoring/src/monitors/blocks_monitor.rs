// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Instant;

use crate::handlers::handler_messages::BlockMetrics;

#[derive(Debug, Clone)]
pub struct BlocksMonitor {
    threshold: usize,
    level: usize,
    downloaded_blocks: usize,
    applied_blocks: usize,
    downloading_group: usize,
    group_download_start: Instant,
    durations: Vec<Option<f32>>,
}

impl BlocksMonitor {
    pub fn new(threshold: usize, downloaded_blocks: usize) -> Self {
        Self {
            threshold,
            level: downloaded_blocks,
            downloaded_blocks,
            applied_blocks: 0,
            downloading_group: downloaded_blocks / threshold,
            group_download_start: Instant::now(),
            durations: vec![None; downloaded_blocks / threshold],
        }
    }

    pub fn accept_block(&mut self) {
        self.level += 1;
    }

    pub fn block_finished_downloading_operations(&mut self) {
        self.downloaded_blocks += 1;
    }

    pub fn block_was_applied_by_protocol(&mut self) {
        self.applied_blocks += 1;
    }

    pub fn snapshot(&mut self) -> Vec<BlockMetrics> {
        use std::cmp::min;

        let group_count = self.level / self.threshold;
        let mut total_count: i32 = self.level as i32;
        let mut downloaded_count: i32 = self.downloaded_blocks as i32;
        let mut applied_count: i32 = self.applied_blocks as i32;
        let mut payload: Vec<BlockMetrics> = Vec::with_capacity(group_count + 1);

        for i in 0..group_count {
            let blocks = min(self.threshold as i32, total_count);
            total_count -= blocks;
            let finished_blocks = min(self.threshold as i32, downloaded_count);
            downloaded_count -= finished_blocks;
            let applied_blocks = min(self.threshold as i32, applied_count);
            applied_count -= applied_blocks;

            if i == self.downloading_group && blocks <= finished_blocks {
                self.downloading_group += 1;
                self.durations
                    .push(Some(self.group_download_start.elapsed().as_secs_f32()));
                self.group_download_start = Instant::now();
            }

            let download_duration = if i < self.durations.len() {
                self.durations[i]
            } else {
                None
            };

            payload.push(BlockMetrics::new(
                i as i32,
                blocks,
                finished_blocks,
                applied_blocks,
                download_duration,
            ));
        }

        payload.push(BlockMetrics::new(
            group_count as i32,
            total_count,
            downloaded_count,
            applied_count,
            None,
        ));

        payload
    }
}
