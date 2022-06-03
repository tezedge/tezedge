use std::time::Instant;

use serde::Serialize;

// cycle is used to measure time
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cycle {
    // cycle id
    id: usize,
    // number of downloaded block headers per cycle
    headers: usize,
    // number of downloaded block operatios per cycle
    operations: usize,
    // number of applied blocks
    applications: usize,
    // skip serialization to JSON for ws
    #[serde(skip_serializing)]
    // timestamp for fisrt block header occurrence
    start: Instant,
    // time to download headers and operations for cycle
    duration: Option<f32>,
}

// monitoring statistics for bootraping
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainMonitor {
    // cycle is used to measure time
    chain: Vec<Cycle>,
}

impl Default for ChainMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl ChainMonitor {
    const BLOCKS_PER_CYCLE: usize = 4096;

    pub fn new() -> Self {
        Self { chain: Vec::new() }
    }

    // get cycle id number
    fn cycle_id(&self, block_level: i32) -> usize {
        block_level as usize / Self::BLOCKS_PER_CYCLE
    }

    // extend chain to support new cycles
    fn update_chain(&mut self, block_level: i32) {
        // cycle number
        let cycle_id = self.cycle_id(block_level);
        // get number of cycles in chain
        let chain_len = self.chain.len();
        // create all missing cycles for chain stats
        if cycle_id >= chain_len {
            let mut chain_append = (chain_len..=cycle_id)
                .map(|id| -> Cycle {
                    Cycle {
                        id: chain_len + id,
                        headers: 0,
                        operations: 0,
                        applications: 0,
                        start: Instant::now(),
                        duration: None,
                    }
                })
                .collect();
            // append created chain stats
            self.chain.append(&mut chain_append);
        };
    }

    // process block header
    pub fn process_block_header(&mut self, block_level: i32) {
        // increment block headers value
        self.get_cycle_for_block_mut(block_level).headers += 1;
    }

    // process block operations
    pub fn process_block_operations(&mut self, block_level: i32) {
        // increment block operations value
        let cycle = self.get_cycle_for_block_mut(block_level);
        cycle.operations += 1;
        // save duration for cycle download time at end of cycle
        if cycle.operations == Self::BLOCKS_PER_CYCLE {
            // save duration in seconds
            cycle.duration = Some(cycle.start.elapsed().as_secs_f32())
        }
    }

    // process block applications
    pub fn process_block_application(&mut self, block_level: i32) {
        // increment block applications value
        self.get_cycle_for_block_mut(block_level).applications += 1;
    }

    #[inline]
    pub fn get_cycle_for_block_mut(&mut self, block_level: i32) -> &mut Cycle {
        // extend chain to support new cycles
        self.update_chain(block_level);
        // cycle number
        let cycle_id = self.cycle_id(block_level);
        // use reference to update cycle stats
        &mut self.chain[cycle_id as usize]
    }

    // snapshot for chain stats
    pub fn snapshot(&self) -> Self {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use crate::monitors::chain_monitor::ChainMonitor;
    use rand::Rng;

    // number of block
    const NUMBER_OF_BLOCKS: i32 = 5000;
    // number of shell events
    const NUMBER_OF_BLOCK_EVENTS: i32 = 10000;

    fn generate_blocks() -> ChainMonitor {
        // start random number generator
        let mut rng = rand::thread_rng();

        // initialize chain_monitor
        let mut chain_monitor = ChainMonitor::new();

        // generate blocks with random level
        for _index in 0..NUMBER_OF_BLOCK_EVENTS {
            // generate block level
            let block_level: i32 = rng.gen_range(0, NUMBER_OF_BLOCKS);

            // process new block header
            chain_monitor.process_block_header(block_level);
            // process new block operations
            chain_monitor.process_block_operations(block_level);
        }

        chain_monitor
    }

    #[test]
    fn process_blocks() {
        // generate block
        let chain_monitor = generate_blocks();

        // check if we have duration in cycle  for block processing time
        assert!(chain_monitor.snapshot().chain[0].duration.is_some());
    }
}
