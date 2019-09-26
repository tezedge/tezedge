use crate::handlers::handler_messages::BlockMetrics;

#[derive(Default, Debug, Clone)]
pub struct BlocksMonitor {
    threshold: usize,
    blocks: usize,
    downloaded_blocks: usize,
    applied_blocks: usize,
}

impl BlocksMonitor {
    pub fn new(threshold: usize) -> Self {
        Self {
            threshold,
            ..Default::default()
        }
    }

    pub fn accept_block(&mut self, _block: i32) {
        self.blocks += 1;
    }

    pub fn block_finished_downloading_operations(&mut self, _block: i32) {
        self.downloaded_blocks += 1;
    }

    pub fn block_was_applied_by_protocol(&mut self, _block: i32) {
        self.applied_blocks += 1;
    }

    pub fn snapshot(&mut self) -> Vec<BlockMetrics> {
        use std::cmp::min;

        let group_count = self.blocks / self.threshold;
        let mut total_count: i32 = self.blocks.clone() as i32;
        let mut downloaded_count: i32 = (self.downloaded_blocks / self.threshold) as i32;
        let mut applied_count: i32 = (self.applied_blocks / self.threshold) as i32;
        let mut payload: Vec<BlockMetrics> = Vec::with_capacity(group_count + 1);

        for i in 0..group_count {
            let blocks = min(self.threshold as i32, total_count);
            total_count -= blocks;
            let finished_blocks = min(self.threshold as i32, downloaded_count);
            downloaded_count -= finished_blocks;
            let applied_blocks = min(self.threshold as i32, applied_count);
            applied_count -= applied_blocks;
            payload.push(BlockMetrics::new(i as i32, blocks, finished_blocks, applied_blocks));
        }
        payload.push(BlockMetrics::new(group_count as i32, total_count, downloaded_count, applied_count));

        payload
    }
}