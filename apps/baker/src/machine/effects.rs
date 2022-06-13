// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::TryFrom, mem, time::Duration};

use redux_rs::{ActionWithMeta, Store, TimeService};

use crypto::hash::{BlockHash, BlockPayloadHash, NonceHash, OperationHash, OperationListHash};
use serde::Deserialize;
use tenderbake as tb;
use tezos_encoding::{enc::BinWriter, types::SizedBytes};
use tezos_messages::{
    p2p::{
        binary_message::MessageHash,
        encoding::operation::{DecodedOperation, Operation},
    },
    protocol::{
        proto_005::operation::SeedNonceRevelationOperation,
        proto_012::operation::{
            Contents, InlinedEndorsementMempoolContents, InlinedPreendorsementContents,
        },
    },
};

use crate::{
    machine::state::Initialized,
    proof_of_work::guess_proof_of_work,
    services::{client::RpcErrorInner, event::OperationSimple, BakerService},
};

use super::{actions::*, state::BakerState};

pub fn baker_effects<S, Srv, A>(store: &mut Store<S, Srv, A>, action: &ActionWithMeta<A>)
where
    S: AsRef<Option<BakerState>>,
    Srv: TimeService + BakerService,
    A: AsRef<Option<BakerAction>> + From<BakerAction>,
{
    let act = action.action.as_ref();

    if act.as_ref().map(|a| a.is_event()).unwrap_or(false) {
        if let Some(baker_state) = store.state.get().as_ref() {
            let to_dispatch = baker_state.as_ref().actions.clone();
            for action in to_dispatch {
                store.dispatch(action);
            }
        }
    }

    let st = store.state.get().as_ref().as_ref().unwrap().as_ref();

    match act {
        // not our action
        None => (),
        // don't handle events here
        Some(
            BakerAction::RpcError(_)
            | BakerAction::IdleEvent(_)
            | BakerAction::ProposalEvent(_)
            | BakerAction::SlotsEvent(_)
            | BakerAction::OperationsForBlockEvent(_)
            | BakerAction::LiveBlocksEvent(_)
            | BakerAction::OperationsEvent(_)
            | BakerAction::TickEvent(_),
        ) => (),
        Some(BakerAction::Idle(IdleAction {})) => {
            store.dispatch(BakerAction::IdleEvent(IdleEventAction {}));
        }
        Some(BakerAction::LogError(LogErrorAction { description })) => {
            slog::error!(store.service.log(), " .  {description}");
        }
        Some(BakerAction::LogWarning(LogWarningAction { description })) => {
            slog::warn!(store.service.log(), " .  {description}");
        }
        Some(BakerAction::LogInfo(LogInfoAction {
            with_prefix: false,
            description,
        })) => {
            slog::info!(store.service.log(), "{description}");
        }
        Some(BakerAction::LogInfo(LogInfoAction {
            with_prefix: true,
            description,
        })) => {
            slog::info!(store.service.log(), " .  {description}");
        }
        Some(BakerAction::LogTenderbake(LogTenderbakeAction { record })) => match record.level() {
            tb::LogLevel::Info => slog::info!(store.service.log(), "{record}"),
            tb::LogLevel::Warn => slog::warn!(store.service.log(), "{record}"),
        },
        Some(BakerAction::GetSlots(GetSlotsAction { level })) => {
            let level = *level;
            match store.service.client().validators(level) {
                Ok(delegates) => {
                    store.dispatch(BakerAction::from(SlotsEventAction { level, delegates }))
                }
                Err(err) => store.dispatch(BakerAction::from(RpcErrorAction {
                    error: err.to_string(),
                })),
            };
        }
        Some(BakerAction::GetOperationsForBlock(GetOperationsForBlockAction { block_hash })) => {
            let block_hash = block_hash.clone();
            match store.service.client().get_operations_for_block(&block_hash) {
                Ok(operations) => {
                    store.dispatch(BakerAction::from(OperationsForBlockEventAction {
                        block_hash,
                        operations,
                    }))
                }
                Err(err) => store.dispatch(BakerAction::from(RpcErrorAction {
                    error: err.to_string(),
                })),
            };
        }
        Some(BakerAction::GetLiveBlocks(GetLiveBlocksAction { block_hash })) => {
            let block_hash = block_hash.clone();
            match store.service.client().get_live_blocks(&block_hash) {
                Ok(live_blocks) => store.dispatch(BakerAction::from(LiveBlocksEventAction {
                    block_hash,
                    live_blocks,
                })),
                Err(err) => store.dispatch(BakerAction::from(RpcErrorAction {
                    error: err.to_string(),
                })),
            };
        }
        Some(BakerAction::MonitorOperations(MonitorOperationsAction {})) => {
            // TODO: investigate it
            let mut tries = 3;
            while tries > 0 {
                tries -= 1;
                if let Err(err) = store
                    .service
                    .client()
                    .monitor_operations(Duration::from_secs(3600))
                {
                    slog::error!(store.service.log(), " .  {err}");
                } else {
                    break;
                }
            }
        }
        Some(BakerAction::ScheduleTimeout(ScheduleTimeoutAction { deadline })) => {
            store.service.timer().schedule(
                *deadline,
                st.tb_state.level().unwrap_or(1),
                st.tb_state.round().unwrap_or(0),
            );
        }
        Some(BakerAction::RevealNonce(RevealNonceAction {
            branch,
            level,
            nonce,
        })) => {
            let content = Contents::SeedNonceRevelation(SeedNonceRevelationOperation {
                level: *level,
                nonce: SizedBytes(nonce.as_slice().try_into().unwrap()),
            });
            let mut bytes = branch.0.clone();
            content.bin_write(&mut bytes).unwrap();
            bytes.extend_from_slice(&[0; 64]);
            let op_hex = hex::encode(bytes);
            match store
                .service
                .client()
                .inject_operation(&st.chain_id, op_hex, false)
            {
                Ok(hash) => slog::info!(store.service.log(), " .  inject nonce_reveal: {hash}"),
                Err(err) => slog::error!(store.service.log(), " .  {err}"),
            }
        }
        Some(BakerAction::PreVote(PreVoteAction { op })) => {
            let InlinedPreendorsementContents::Preendorsement(c) = &op.operations;
            let (data, _) =
                match store
                    .service
                    .crypto()
                    .sign(0x12, &st.chain_id, op, c.level, c.round)
                {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(store.service.log(), " .  {err}");
                        return;
                    }
                };
            match store
                .service
                .client()
                .inject_operation(&st.chain_id, hex::encode(data), false)
            {
                Ok(hash) => slog::info!(store.service.log(), " .  inject preendorsement: {hash}"),
                Err(err) => slog::error!(store.service.log(), " .  {err}"),
            }
        }
        Some(BakerAction::Vote(VoteAction { op })) => {
            let InlinedEndorsementMempoolContents::Endorsement(c) = &op.operations;
            let (data, _) =
                match store
                    .service
                    .crypto()
                    .sign(0x13, &st.chain_id, op, c.level, c.round)
                {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(store.service.log(), " .  {err}");
                        return;
                    }
                };
            match store
                .service
                .client()
                .inject_operation(&st.chain_id, hex::encode(data), false)
            {
                Ok(hash) => slog::info!(store.service.log(), " .  inject endorsement: {hash}"),
                Err(err) => slog::error!(store.service.log(), " .  {err}"),
            }
        }
        Some(BakerAction::Propose(ProposeAction {
            payload_round,
            seed_nonce_hash,
            predecessor_hash,
            operations,
            timestamp,
            round,
            level,
        })) => inject_block(
            &mut store.service,
            st,
            *payload_round,
            seed_nonce_hash,
            predecessor_hash,
            operations,
            timestamp,
            *round,
            *level,
        ),
    }
}

fn inject_block<Srv>(
    srv: &mut Srv,
    st: &Initialized,
    payload_round: i32,
    seed_nonce_hash: &Option<NonceHash>,
    predecessor_hash: &BlockHash,
    operations: &[Vec<OperationSimple>; 4],
    timestamp: &i64,
    round: i32,
    level: i32,
) where
    Srv: TimeService + BakerService,
{
    let payload_hash = {
        let hashes = operations[1..]
            .as_ref()
            .iter()
            .flatten()
            .filter_map(|op| op.hash.as_ref().cloned())
            .collect::<Vec<_>>();
        let operation_list_hash = OperationListHash::calculate(&hashes).unwrap();
        BlockPayloadHash::calculate(
            &predecessor_hash,
            payload_round as u32,
            &operation_list_hash,
        )
        .unwrap()
    };

    let sum_before = operations[1..]
        .iter()
        .map(|ops_list| ops_list.len())
        .sum::<usize>();

    let (mut header, ops) = match srv.client().preapply_block(
        payload_hash,
        payload_round,
        seed_nonce_hash,
        predecessor_hash.clone(),
        *timestamp,
        operations.clone(),
    ) {
        Ok(v) => v,
        Err(err) => {
            slog::error!(srv.log(), " .  {err}");
            return;
        }
    };

    let valid_operations = ops
        .iter()
        .filter_map(|v| {
            let applied = v.as_object()?.get("applied")?.clone();
            serde_json::from_value(applied).ok()
        })
        .collect::<Vec<Vec<DecodedOperation>>>();
    let sum = valid_operations[1..]
        .iter()
        .map(|ops_list| ops_list.len())
        .sum::<usize>();
    if sum != sum_before || payload_round != header.payload_round {
        let hashes = valid_operations[1..]
            .as_ref()
            .iter()
            .flatten()
            .filter_map(|op| match Operation::try_from(op.clone()) {
                Ok(op) => match op.message_typed_hash() {
                    Ok(v) => Some(v),
                    Err(err) => {
                        slog::error!(srv.log(), "calculate hash: {err}");
                        None
                    }
                },
                Err(err) => {
                    slog::error!(srv.log(), "decoded operation: {err}");
                    None
                }
            })
            .collect::<Vec<_>>();
        let operation_list_hash = OperationListHash::calculate(&hashes).unwrap();
        header.payload_hash = BlockPayloadHash::calculate(
            &predecessor_hash,
            payload_round as u32,
            &operation_list_hash,
        )
        .unwrap();
    }

    // is the offset of `proof_of_work_nonce` in protocol header (Ithaca and Jakarta)
    let nonce_offset = mem::size_of::<BlockPayloadHash>() + mem::size_of::<i32>();
    let p = guess_proof_of_work(&header, nonce_offset, st.proof_of_work_threshold);
    header.proof_of_work_nonce = SizedBytes(p);
    slog::info!(srv.log(), "{:?}", header);
    let (data, _) = match srv.crypto().sign(0x11, &st.chain_id, &header, level, round) {
        Ok(v) => v,
        Err(err) => {
            slog::error!(srv.log(), " .  {err}");
            return;
        }
    };

    match srv
        .client()
        .inject_block(hex::encode(data), valid_operations)
    {
        Ok(hash) => slog::info!(
            srv.log(),
            " .  inject block: {}:{}, {hash}",
            header.level,
            round
        ),
        Err(err) => {
            match err.as_ref() {
                RpcErrorInner::NodeError(err, _) => {
                    let invalid_ops = extract_invalid_ops(&err);
                    if !invalid_ops.is_empty() {
                        let mut operations = operations.clone();
                        for ops_list in &mut operations {
                            ops_list.retain(|op| match &op.hash {
                                None => true,
                                Some(hash) => !invalid_ops.contains(hash),
                            });
                        }
                        // try again recursively
                        inject_block(
                            srv,
                            st,
                            header.payload_round,
                            seed_nonce_hash,
                            predecessor_hash,
                            &operations,
                            timestamp,
                            round,
                            level,
                        );

                        return;
                    }
                }
                _ => (),
            }
            slog::error!(srv.log(), " .  {err}");
            slog::error!(srv.log(), " .  {}", serde_json::to_string(&ops).unwrap());
        }
    }
}

fn extract_invalid_ops(err: &str) -> Vec<OperationHash> {
    #[derive(Deserialize)]
    struct NodeError {
        error: String,
        operation: OperationHash,
    }

    let mut ops = vec![];
    if let Ok(errors) = serde_json::from_str::<Vec<NodeError>>(err) {
        for err in errors {
            if err.error == "outdated_operation" {
                ops.push(err.operation);
            }
        }
    }
    ops
}

#[cfg(test)]
#[test]
fn extract_invalid_ops_test() {
    let err = r#"[{"kind":"permanent","id":"validator.invalid_block","invalid_block":"BM8EPrBCqLzxNxqWkctjREMqNzjfiHNFMssvS4fhJXZLeLTZTf6","error":"outdated_operation","operation":"oo5qFAHchTG9rpNDbRBmSaJbSsiCqHBtfmKwk9Atiavhih9hYbC","originating_block":"BMExe9wTeATNPfXpymKc7iCLD3oDrw1zHCwXTVK261qf9MevmAo"}]"#;
    let ops = extract_invalid_ops(err);
    assert_eq!(ops.len(), 1);
}

#[cfg(test)]
#[test]
fn ph_() {
    let predecessor = BlockHash::from_base58_check("BM76V7ruQpqqbo3qZXjAJgMdhD3RD42NkjA1YCAcJyNnbmE6o9L").unwrap();
    let operation_list_hash = OperationListHash::calculate(&[]).unwrap();
    let payload_hash = BlockPayloadHash::calculate(&predecessor, 0, &operation_list_hash).unwrap();
    println!("{payload_hash}");
}
