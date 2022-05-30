// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{convert::TryInto, time::Duration};

use redux_rs::{ActionWithMeta, Store, TimeService};

use tenderbake as tb;
use tezos_encoding::{enc::BinWriter, types::SizedBytes};
use tezos_messages::protocol::{
    proto_005::operation::SeedNonceRevelationOperation, proto_012::operation::Contents,
};

use crate::{proof_of_work::guess_proof_of_work, services::BakerService};

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
            let (data, _) = store.service.crypto().sign(0x12, &st.chain_id, op).unwrap();
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
            let (data, _) = store.service.crypto().sign(0x13, &st.chain_id, op).unwrap();
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
            protocol_header,
            predecessor_hash,
            operations,
            timestamp,
            round,
        })) => {
            let (_, signature) = store
                .service
                .crypto()
                .sign(0x11, &st.chain_id, protocol_header)
                .unwrap();
            let mut protocol_header = protocol_header.clone();
            protocol_header.signature = signature;

            let (mut header, ops) = match store.service.client().preapply_block(
                protocol_header,
                predecessor_hash.clone(),
                *timestamp,
                operations.clone(),
            ) {
                Ok(v) => v,
                Err(err) => {
                    slog::error!(store.service.log(), " .  {err}");
                    return;
                }
            };

            header.signature.0 = vec![0x00; 64];
            let p = guess_proof_of_work(&header, st.proof_of_work_threshold);
            header.proof_of_work_nonce = SizedBytes(p);
            slog::info!(store.service.log(), "{:?}", header);
            let (data, _) = store
                .service
                .crypto()
                .sign(0x11, &st.chain_id, &header)
                .unwrap();

            let valid_operations = ops
                .iter()
                .filter_map(|v| {
                    let applied = v.as_object()?.get("applied")?.clone();
                    serde_json::from_value(applied).ok()
                })
                .collect();

            match store
                .service
                .client()
                .inject_block(hex::encode(data), valid_operations)
            {
                Ok(hash) => slog::info!(
                    store.service.log(),
                    " .  inject block: {}:{}, {hash}",
                    header.level,
                    round
                ),
                Err(err) => {
                    slog::error!(store.service.log(), " .  {err}");
                    slog::error!(
                        store.service.log(),
                        " .  {}",
                        serde_json::to_string(&ops).unwrap()
                    );
                }
            }
        }
    }
}
