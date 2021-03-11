// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::AddAssign;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use getset::Getters;

use tezos_messages::p2p::encoding::block_header::Level;

/// Shareabale type for stats
pub type ApplyBlockStatsRef = Arc<RwLock<ApplyBlockStats>>;

/// Inits empty current head state
pub fn init_empty_apply_block_stats() -> ApplyBlockStatsRef {
    Arc::new(RwLock::new(ApplyBlockStats::default()))
}

/// Statistics about applying
#[derive(Getters)]
pub struct ApplyBlockStats {
    /// ID of the last applied block
    #[get = "pub(crate)"]
    applied_block_level: Option<Level>,
    /// Last time a block was applied
    #[get = "pub(crate)"]
    applied_block_last: Option<Instant>,

    /// Count of applied blocks, from last LogStats run
    #[get = "pub(crate)"]
    applied_block_lasts_count: u32,
    /// Sum of durations of block validation with protocol from last LogStats run
    #[get = "pub(crate)"]
    applied_block_lasts_sum_validation_timer: BlockValidationTimer,
}

impl Default for ApplyBlockStats {
    fn default() -> Self {
        Self {
            applied_block_level: None,
            applied_block_last: None,
            applied_block_lasts_count: 0,
            applied_block_lasts_sum_validation_timer: BlockValidationTimer::default(),
        }
    }
}

impl ApplyBlockStats {
    pub fn clear_applied_block_lasts(&mut self) {
        self.applied_block_lasts_count = 0;
        self.applied_block_lasts_sum_validation_timer = BlockValidationTimer::default();
    }

    pub fn add_block_validation_stats(&mut self, validation_timer: Arc<BlockValidationTimer>) {
        self.applied_block_lasts_count += 1;
        self.applied_block_lasts_sum_validation_timer
            .add_assign(validation_timer);
    }

    pub fn set_applied_block_level(&mut self, new_level: Level) {
        self.applied_block_level = Some(new_level);
        self.applied_block_last = Some(Instant::now());
    }
}

#[derive(Clone, Debug)]
pub struct BlockValidationTimer {
    validated_at: Duration,
    load_metadata_elapsed: Duration,
    protocol_call_elapsed: Duration,
    context_wait_elapsed: Duration,
    store_result_elapsed: Duration,
}

impl BlockValidationTimer {
    pub fn new(
        validated_at: Duration,
        load_metadata_elapsed: Duration,
        protocol_call_elapsed: Duration,
        context_wait_elapsed: Duration,
        store_result_elapsed: Duration,
    ) -> Self {
        Self {
            validated_at,
            load_metadata_elapsed,
            protocol_call_elapsed,
            context_wait_elapsed,
            store_result_elapsed,
        }
    }

    pub fn print_formatted_average_for_count(&self, count: u32) -> String {
        let div = |duration: Duration, count: u32| -> String {
            match duration.checked_div(count) {
                Some(result) => format!("{:?}", result),
                None => "-".to_string(),
            }
        };

        format!(
            "validation {} -> load_metadata {} + protocol_call {} + context_check {} + store_result {}",
            div(self.validated_at, count),
            div(self.load_metadata_elapsed, count),
            div(self.protocol_call_elapsed, count),
            div(self.context_wait_elapsed, count),
            div(self.store_result_elapsed, count),
        )
    }
}

impl AddAssign<Arc<BlockValidationTimer>> for BlockValidationTimer {
    fn add_assign(&mut self, rhs: Arc<BlockValidationTimer>) {
        self.validated_at = match self.validated_at.checked_add(rhs.validated_at) {
            Some(result) => result,
            None => self.validated_at,
        };
        self.load_metadata_elapsed = match self
            .load_metadata_elapsed
            .checked_add(rhs.load_metadata_elapsed)
        {
            Some(result) => result,
            None => self.load_metadata_elapsed,
        };
        self.protocol_call_elapsed = match self
            .protocol_call_elapsed
            .checked_add(rhs.protocol_call_elapsed)
        {
            Some(result) => result,
            None => self.protocol_call_elapsed,
        };
        self.context_wait_elapsed = match self
            .context_wait_elapsed
            .checked_add(rhs.context_wait_elapsed)
        {
            Some(result) => result,
            None => self.context_wait_elapsed,
        };
        self.store_result_elapsed = match self
            .store_result_elapsed
            .checked_add(rhs.store_result_elapsed)
        {
            Some(result) => result,
            None => self.store_result_elapsed,
        };
    }
}

impl Default for BlockValidationTimer {
    fn default() -> Self {
        Self::new(
            Duration::new(0, 0),
            Duration::new(0, 0),
            Duration::new(0, 0),
            Duration::new(0, 0),
            Duration::new(0, 0),
        )
    }
}
