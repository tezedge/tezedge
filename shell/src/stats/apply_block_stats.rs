// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::ops::AddAssign;
use std::time::{Duration, Instant};

use getset::Getters;

use tezos_messages::p2p::encoding::block_header::Level;

/// Statistics about applying
#[derive(Getters, Clone, Debug, Default)]
pub struct ApplyBlockStats {
    /// ID of the last applied block
    #[get = "pub(crate)"]
    applied_block_level: Option<Level>,
    /// Last time a block was applied
    #[get = "pub(crate)"]
    applied_block_last: Option<Instant>,

    /// Count of applied blocks, from the last clearing
    #[get = "pub(crate)"]
    applied_block_lasts_count: u32,
    /// Sum of durations of block validation with protocol from last LogStats run
    applied_block_lasts_sum_validation_timer: BlockValidationTimer,
}

impl ApplyBlockStats {
    pub fn clear_applied_block_lasts(&mut self) {
        self.applied_block_lasts_count = 0;
        self.applied_block_lasts_sum_validation_timer = BlockValidationTimer::default();
    }

    pub fn add_block_validation_stats(&mut self, validation_timer: &BlockValidationTimer) {
        self.applied_block_lasts_count += 1;
        self.applied_block_lasts_sum_validation_timer
            .add_assign(validation_timer);
    }

    pub fn sum_validated_at_time(&self) -> &Duration {
        &self.applied_block_lasts_sum_validation_timer.validated_at
    }

    pub fn print_formatted_average_times(&self) -> String {
        let div = |duration: Duration, count: u32| -> String {
            match duration.checked_div(count) {
                Some(result) => format!("{:?}", result),
                None => "-".to_string(),
            }
        };

        format!(
            "validation {} -> load_metadata {} + protocol_call {} + store_result {}",
            div(
                self.applied_block_lasts_sum_validation_timer.validated_at,
                self.applied_block_lasts_count
            ),
            div(
                self.applied_block_lasts_sum_validation_timer
                    .load_metadata_elapsed,
                self.applied_block_lasts_count
            ),
            div(
                self.applied_block_lasts_sum_validation_timer
                    .protocol_call_elapsed,
                self.applied_block_lasts_count
            ),
            div(
                self.applied_block_lasts_sum_validation_timer
                    .store_result_elapsed,
                self.applied_block_lasts_count
            ),
        )
    }

    pub fn set_applied_block_level(&mut self, new_level: Level) {
        self.applied_block_level = Some(new_level);
        self.applied_block_last = Some(Instant::now());
    }

    pub fn merge(&mut self, new_stats: ApplyBlockStats) {
        self.applied_block_last = new_stats.applied_block_last;
        self.applied_block_level = new_stats.applied_block_level;
        self.applied_block_lasts_count += new_stats.applied_block_lasts_count;
        self.applied_block_lasts_sum_validation_timer
            .add_assign(&new_stats.applied_block_lasts_sum_validation_timer);
    }
}

#[derive(Clone, Debug)]
pub struct BlockValidationTimer {
    validated_at: Duration,
    load_metadata_elapsed: Duration,
    protocol_call_elapsed: Duration,
    store_result_elapsed: Duration,
}

impl BlockValidationTimer {
    pub fn new(
        validated_at: Duration,
        load_metadata_elapsed: Duration,
        protocol_call_elapsed: Duration,
        store_result_elapsed: Duration,
    ) -> Self {
        Self {
            validated_at,
            load_metadata_elapsed,
            protocol_call_elapsed,
            store_result_elapsed,
        }
    }
}

impl AddAssign<&BlockValidationTimer> for BlockValidationTimer {
    fn add_assign(&mut self, rhs: &BlockValidationTimer) {
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
        )
    }
}
