use crate::handlers::handler_messages::BlockMetrics;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct BlocksMonitor {
    threshold: usize,
    blocks: usize,
    downloaded_blocks: usize,
    applied_blocks: usize,
    created_at: Vec<Instant>,
    durations: Vec<f32>,
}

impl BlocksMonitor {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            blocks: 0,
            downloaded_blocks: 0,
            applied_blocks: 0,
            created_at: Vec::new(),
            durations: Vec::new(),
        }
    }

    pub fn accept_block(&mut self) {
        self.blocks += 1;
    }

    pub fn block_finished_downloading_operations(&mut self) {
        self.downloaded_blocks += 1;
    }

    pub fn block_was_applied_by_protocol(&mut self) {
        self.applied_blocks += 1;
    }

    pub fn snapshot(&mut self) -> Vec<BlockMetrics> {
        use std::cmp::min;

        let group_count = self.blocks / self.threshold;
        let mut total_count: i32 = self.blocks.clone() as i32;
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
            let download_duration = if i < self.created_at.len() {
                if finished_blocks != blocks {
                    None
                } else {
                    if i >= self.durations.len() {
                        self.durations.push(self.created_at[i].elapsed().as_secs_f32());
                    }
                    Some(self.durations[i])
                }
            } else {
                self.created_at.push(Instant::now());
                None
            };
            payload.push(BlockMetrics::new(i as i32, blocks, finished_blocks, applied_blocks, download_duration));
        }
        let i = group_count;
        let download_duration = if i < self.created_at.len() {
            if self.threshold != downloaded_count as usize {
                None
            } else {
                if i >= self.durations.len() {
                    self.durations.push(self.created_at[i].elapsed().as_secs_f32());
                }
                Some(self.durations[i])
            }
        } else {
            self.created_at.push(Instant::now());
            None
        };
        payload.push(BlockMetrics::new(group_count as i32, total_count, downloaded_count, applied_count, download_duration));

        payload
    }
}