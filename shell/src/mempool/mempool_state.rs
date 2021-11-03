// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use crypto::hash::{BlockHash, OperationHash};
use tezos_api::ffi::{PrevalidatorWrapper, ValidateOperationResult};
use tezos_messages::p2p::encoding::prelude::Mempool;

use shell_integration::{MempoolOperationRef, StreamCounter, StreamWakers};
/// Mempool state is defined with mempool and validation_result attributes, which are in sync:
/// - `validation_result`
///     - contains results of all validated operations
///     - also contains `known_valid` operations, which where validated as `applied`
/// - `pending`
///     - operations, which where not validated yet or endorsements (`branch_refused`, `branched_delay`, `refused`?)
///     - are being processed sequentially, after validation, they are moved to `validation_result`
/// - `operations`
///     - kind of cache, contains operation data
#[derive(Debug, Default)]
pub struct MempoolState {
    /// Original tezos prevalidator has prevalidator.fitness which is used for set_head comparision
    /// So, we keep it in-memory here
    prevalidator: Option<PrevalidatorWrapper>,
    prevalidator_started: Option<DateTime<Utc>>,
    predecessor: Option<BlockHash>,

    /// Actual cumulated operation results
    validation_result: ValidateOperationResult,

    /// In-memory store of actual operations
    operations: HashMap<OperationHash, MempoolOperationRef>,
    // TODO: pendings limit
    // TODO: pendings as vec and order
    pending: HashSet<OperationHash>,

    // Wakers for open streams (monitors) that access the mempool state
    streams: StreamWakers,
}

impl StreamCounter for MempoolState {
    fn get_streams(&self) -> &StreamWakers {
        &self.streams
    }

    fn get_mutable_streams(&mut self) -> &mut StreamWakers {
        &mut self.streams
    }
}

impl MempoolState {
    /// Reinitialize state for new prevalidator and head, returns unneeded operation hashes
    pub(crate) fn reinit(
        &mut self,
        prevalidator: Option<PrevalidatorWrapper>,
        predecessor: Option<BlockHash>,
    ) -> Vec<OperationHash> {
        // we want to validate pending operations with new prevalidator, so other "already_validated" can be removed
        let unneeded_operations: Vec<OperationHash> = self
            .operations
            .keys()
            .filter(|&key| !self.pending.contains(key))
            .cloned()
            .collect();

        // remove unneeded
        for oph in &unneeded_operations {
            self.operations.remove(oph);
        }
        self.predecessor = predecessor;
        self.prevalidator = prevalidator;
        self.validation_result = ValidateOperationResult::default();

        self.wake_up_all_streams();

        unneeded_operations
    }

    /// Tries to add operation to pendings.
    /// Returns true - if added, false - if operation was already validated
    pub(crate) fn add_to_pending(
        &mut self,
        operation_hash: &OperationHash,
        operation: MempoolOperationRef,
    ) -> bool {
        if self.is_already_validated(operation_hash) {
            return false;
        }

        if self.pending.contains(operation_hash) {
            false
        } else {
            self.operations.insert(operation_hash.clone(), operation);
            self.pending.insert(operation_hash.clone())
        }
    }

    /// Removes operation from mempool
    pub fn remove_operation(&mut self, oph: OperationHash) {
        // remove from applied
        if let Some(pos) = self
            .validation_result
            .applied
            .iter()
            .position(|x| oph.eq(&x.hash))
        {
            self.validation_result.applied.remove(pos);
            self.operations.remove(&oph);
        }
        // remove from branch_delayed
        if let Some(pos) = self
            .validation_result
            .branch_delayed
            .iter()
            .position(|x| oph.eq(&x.hash))
        {
            self.validation_result.branch_delayed.remove(pos);
            self.operations.remove(&oph);
        }
        // remove from branch_refused
        if let Some(pos) = self
            .validation_result
            .branch_refused
            .iter()
            .position(|x| oph.eq(&x.hash))
        {
            self.validation_result.branch_refused.remove(pos);
            self.operations.remove(&oph);
        }
        // remove from refused
        if let Some(pos) = self
            .validation_result
            .refused
            .iter()
            .position(|x| oph.eq(&x.hash))
        {
            self.validation_result.refused.remove(pos);
            self.operations.remove(&oph);
        }
        // remove from pending
        if self.pending.contains(&oph) {
            self.pending.remove(&oph);
            self.operations.remove(&oph);
        }
    }

    /// Drain pending operations, that need to be handled
    /// Returns - None, if nothing can be done, or Some(prevalidator, head, pendings, operations) to handle
    pub(crate) fn drain_pending(
        &mut self,
    ) -> Option<(
        PrevalidatorWrapper,
        BlockHash,
        HashSet<OperationHash>,
        HashMap<OperationHash, MempoolOperationRef>,
    )> {
        if self.pending.is_empty() {
            return None;
        }

        let (prevalidator, head) = match self.prevalidator.as_ref() {
            Some(prevalidator) => match self.predecessor.as_ref() {
                Some(head) => (prevalidator.clone(), head.clone()),
                None => return None,
            },
            None => return None,
        };

        // drain pendings to handle
        let pending_to_handle: HashSet<OperationHash> = self.pending.drain().collect();
        let pending_operations: HashMap<OperationHash, MempoolOperationRef> = self
            .operations
            .iter()
            .filter(|(op, _)| pending_to_handle.contains(op))
            .map(|(op, mempool_operation)| (op.clone(), mempool_operation.clone()))
            .collect();

        Some((prevalidator, head, pending_to_handle, pending_operations))
    }

    pub(crate) fn merge_validation_result(&mut self, result: ValidateOperationResult) {
        let _ = self.validation_result.merge(result);
    }

    pub fn collect_mempool_operations_to_advertise(
        &self,
        mut pending: HashSet<OperationHash>,
    ) -> Option<(Mempool, HashMap<OperationHash, MempoolOperationRef>)> {
        // advertise only if we have applied operations
        if self.validation_result.applied.is_empty() {
            return None;
        }

        let mut mempool_operations: HashMap<OperationHash, MempoolOperationRef> =
            HashMap::with_capacity(self.validation_result.applied.len());

        // collect applied as known_valid
        let known_valid: Vec<OperationHash> = self
            .validation_result
            .applied
            .iter()
            .filter_map(|op| {
                // advertise just operation_hash, for which we have data
                if let Some((operation_hash, mempool_operation)) =
                    self.operations.get_key_value(&op.hash)
                {
                    mempool_operations.insert(operation_hash.clone(), mempool_operation.clone());
                    Some(operation_hash.clone())
                } else {
                    None
                }
            })
            .collect();

        // collect all pendings
        self.pending.iter().for_each(|p| {
            let _ = pending.insert(p.clone());
        });

        let pending: Vec<OperationHash> = pending
            .into_iter()
            .filter_map(|op| {
                // advertise just operation_hash, for which we have data
                if let Some((operation_hash, mempool_operation)) =
                    self.operations.get_key_value(&op)
                {
                    mempool_operations.insert(operation_hash.clone(), mempool_operation.clone());
                    Some(op)
                } else {
                    None
                }
            })
            .collect();

        Some((Mempool::new(known_valid, pending), mempool_operations))
    }

    pub fn collect_mempool_to_advertise(&self) -> Option<Mempool> {
        // advertise only if we have applied operations
        if self.validation_result.applied.is_empty() {
            return None;
        }

        // collect applied as known_valid
        let known_valid: Vec<OperationHash> = self
            .validation_result
            .applied
            .iter()
            .filter_map(|op| {
                // advertise just operation_hash, for which we have data
                if self.operations.contains_key(&op.hash) {
                    Some(op.hash.clone())
                } else {
                    None
                }
            })
            .collect();

        // collect all pendings
        let pending: Vec<OperationHash> = self
            .pending
            .iter()
            .filter_map(|op| {
                // advertise just operation_hash, for which we have data
                if self.operations.contains_key(op) {
                    Some(op.clone())
                } else {
                    None
                }
            })
            .collect();

        Some(Mempool::new(known_valid, pending))
    }

    /// Indicates, that the operation was already validated and is in the mempool
    fn is_already_validated(&self, operation_hash: &OperationHash) -> bool {
        if self
            .validation_result
            .applied
            .iter()
            .any(|op| op.hash.eq(operation_hash))
        {
            return true;
        }
        if self
            .validation_result
            .branch_delayed
            .iter()
            .any(|op| op.hash.eq(operation_hash))
        {
            return true;
        }
        if self
            .validation_result
            .branch_refused
            .iter()
            .any(|op| op.hash.eq(operation_hash))
        {
            return true;
        }
        if self
            .validation_result
            .refused
            .iter()
            .any(|op| op.hash.eq(operation_hash))
        {
            return true;
        }
        false
    }

    pub fn is_already_in_mempool(&self, operation_hash: &OperationHash) -> bool {
        self.pending.contains(operation_hash) || self.is_already_validated(operation_hash)
    }

    pub fn prevalidator(&self) -> Option<&PrevalidatorWrapper> {
        self.prevalidator.as_ref()
    }

    pub fn prevalidator_started(&self) -> Option<&DateTime<Utc>> {
        self.prevalidator_started.as_ref()
    }

    pub fn set_prevalidator_started(&mut self) {
        self.prevalidator_started = Some(Utc::now());
    }

    pub fn head(&self) -> Option<&BlockHash> {
        self.predecessor.as_ref()
    }

    pub fn result(&self) -> &ValidateOperationResult {
        &self.validation_result
    }

    pub fn operations(&self) -> &HashMap<OperationHash, MempoolOperationRef> {
        &self.operations
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::sync::Arc;

    use tezos_api::ffi::PrevalidatorWrapper;
    use tezos_messages::p2p::binary_message::BinaryRead;
    use tezos_messages::p2p::encoding::prelude::{Operation, OperationMessage};

    use crate::mempool::MempoolState;

    #[test]
    fn test_state_reinit() -> Result<(), anyhow::Error> {
        let op_hash1 = "opJ4FdKumPfykAP9ZqwY7rNB8y1SiMupt44RqBDMWL7cmb4xbNr".try_into()?;
        let op_hash2 = "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?;

        // init state with two pendings
        let mut state = MempoolState::default();
        state.add_to_pending(
            &op_hash1,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );
        state.add_to_pending(
            &op_hash2,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );
        assert_eq!(2, state.pending.len());
        assert_eq!(2, state.operations.len());

        // no prevalidator/ no head, means nothing to handle
        assert!(state.drain_pending().is_none());

        // add header/prevalidator
        let _ = state.reinit(
            Some(PrevalidatorWrapper {
                chain_id: "NetXgtSLGNJvNye".try_into()?,
                protocol: "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb".try_into()?,
                context_fitness: Some(vec![vec![0, 1], vec![0, 0, 1, 2, 3, 4, 5]]),
            }),
            Some("BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9".try_into()?),
        );

        // remove from pending
        assert_eq!(2, state.pending.len());
        assert_eq!(2, state.operations.len());
        let handle_pendings = state.drain_pending();
        assert!(handle_pendings.is_some());
        let (prevalidator, head, pendings, pendings_operations) = handle_pendings.unwrap();
        assert_eq!(2, pendings.len());
        assert_eq!(0, state.pending.len());
        assert_eq!(2, pendings_operations.len());
        assert_eq!(2, state.operations.len());

        // test remove operation
        state.add_to_pending(
            &op_hash1,
            Arc::new(OperationMessage::from(Operation::from_bytes(hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?)?)),
        );
        assert_eq!(1, state.pending.len());
        assert_eq!(2, state.operations.len());

        state.remove_operation(op_hash1);
        assert!(state.pending.is_empty());
        assert_eq!(1, state.operations.len());

        // reinit
        let unneded = state.reinit(Some(prevalidator), Some(head));
        assert_eq!(1, unneded.len());
        assert!(state.pending.is_empty());
        assert!(state.operations.is_empty());

        // drain should not be touch
        assert_eq!(2, pendings.len());
        assert_eq!(2, pendings_operations.len());

        Ok(())
    }
}
