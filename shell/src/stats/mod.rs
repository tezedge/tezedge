// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module contains all structs used to hold shell stats.

use std::ops::AddAssign;
use std::sync::Arc;
use std::time::Duration;

pub mod memory;

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
