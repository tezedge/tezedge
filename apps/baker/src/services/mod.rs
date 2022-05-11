// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod client;
pub mod event;
pub mod key;
pub mod logger;
pub mod timer;

use std::{
    convert::TryInto,
    path::PathBuf,
    sync::mpsc,
    time::{Duration, SystemTime},
};

use reqwest::Url;

use crypto::hash::{BlockHash, ChainId};
use redux_rs::TimeService;
use tenderbake as tb;
use tezos_encoding::{enc::BinWriter, types::SizedBytes};
use tezos_messages::protocol::{
    proto_005_2::operation::SeedNonceRevelationOperation,
    proto_012::operation::{Contents, InlinedEndorsement, InlinedPreendorsement},
};

use crate::{
    machine::{
        BakerAction, IdleEventAction, LiveBlocksEventAction, OperationsForBlockEventAction,
        RpcErrorAction, SlotsEventAction,
    },
    proof_of_work::guess_proof_of_work,
};

use self::{client::ProtocolBlockHeader, event::OperationSimple};

pub struct Services {
    pub client: client::RpcClient,
    pub crypto: key::CryptoService,
    pub log: slog::Logger,
    timer: timer::Timer,
    sender: mpsc::Sender<BakerAction>,
}

pub struct EventWithTime {
    pub action: BakerAction,
    pub now: tenderbake::Timestamp,
}

// TODO: remove it
#[derive(Clone)]
pub enum ActionInner {
    Idle,
    LogError(String),
    LogWarning(String),
    LogInfo {
        with_prefix: bool,
        description: String,
    },
    LogTb(tb::LogRecord),

    GetSlots {
        level: i32,
    },
    GetOperationsForBlock {
        block_hash: BlockHash,
    },
    GetLiveBlocks {
        block_hash: BlockHash,
    },
    MonitorOperations,
    ScheduleTimeout(tb::Timestamp),
    RevealNonce {
        chain_id: ChainId,
        branch: BlockHash,
        level: i32,
        nonce: Vec<u8>,
    },
    PreVote(ChainId, InlinedPreendorsement),
    Vote(ChainId, InlinedEndorsement),
    Propose {
        chain_id: ChainId,
        proof_of_work_threshold: u64,
        protocol_header: ProtocolBlockHeader,
        predecessor_hash: BlockHash,
        operations: [Vec<OperationSimple>; 4],
        timestamp: i64,
        round: i32,
    },
}

impl Services {
    pub fn new(
        endpoint: Url,
        base_dir: &PathBuf,
        baker: &str,
    ) -> (Self, impl Iterator<Item = EventWithTime>) {
        let (tx, rx) = mpsc::channel();

        let srv = Services {
            client: client::RpcClient::new(endpoint, tx.clone()),
            crypto: key::CryptoService::read_key(base_dir, baker).unwrap(),
            log: logger::main_logger(),
            timer: timer::Timer::spawn(tx.clone()),
            sender: tx,
        };

        slog::info!(
            srv.log,
            "crypto service ready: {}",
            srv.crypto.public_key_hash()
        );

        (
            srv,
            rx.into_iter().map(|event| {
                let unix_epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                let now = tb::Timestamp { unix_epoch };
                EventWithTime { now, action: event }
            }),
        )
    }
}

pub trait BakerService {
    fn execute(&mut self, action: &ActionInner);
}

impl BakerService for Services {
    fn execute(&mut self, action: &ActionInner) {
        let action = action.clone();

        match action {
            ActionInner::Idle => drop(self.sender.send(BakerAction::IdleEvent(IdleEventAction {}))),
            ActionInner::LogError(error) => slog::error!(self.log, " .  {error}"),
            ActionInner::LogWarning(warn) => slog::warn!(self.log, " .  {warn}"),
            ActionInner::LogInfo {
                with_prefix: true,
                description,
            } => {
                slog::info!(self.log, " .  {description}")
            }
            ActionInner::LogInfo {
                with_prefix: false,
                description,
            } => {
                slog::info!(self.log, "{description}")
            }
            ActionInner::LogTb(record) => match record.level() {
                tb::LogLevel::Info => slog::info!(self.log, "{record}"),
                tb::LogLevel::Warn => slog::warn!(self.log, "{record}"),
            },
            ActionInner::GetSlots { level } => match self.client.validators(level) {
                Ok(delegates) => {
                    let _ = self.sender.send(BakerAction::SlotsEvent(SlotsEventAction {
                        level,
                        delegates,
                    }));
                }
                Err(err) => drop(self.sender.send(BakerAction::RpcError(RpcErrorAction {
                    error: err.to_string(),
                }))),
            },
            ActionInner::GetOperationsForBlock { block_hash } => {
                match self.client.get_operations_for_block(&block_hash) {
                    Ok(operations) => {
                        let act = OperationsForBlockEventAction {
                            block_hash,
                            operations,
                        };
                        let _ = self.sender.send(BakerAction::OperationsForBlockEvent(act));
                    }
                    Err(err) => drop(self.sender.send(BakerAction::RpcError(RpcErrorAction {
                        error: err.to_string(),
                    }))),
                }
            }
            ActionInner::GetLiveBlocks { block_hash } => {
                match self.client.get_live_blocks(&block_hash) {
                    Ok(live_blocks) => {
                        let act = LiveBlocksEventAction {
                            block_hash,
                            live_blocks,
                        };
                        let _ = self.sender.send(BakerAction::LiveBlocksEvent(act));
                    }
                    Err(err) => drop(self.sender.send(BakerAction::RpcError(RpcErrorAction {
                        error: err.to_string(),
                    }))),
                }
            }
            ActionInner::MonitorOperations => {
                // TODO: investigate it
                let mut tries = 3;
                while tries > 0 {
                    tries -= 1;
                    if let Err(err) = self.client.monitor_operations(Duration::from_secs(3600)) {
                        slog::error!(self.log, " .  {}", err);
                    } else {
                        break;
                    }
                }
            }
            ActionInner::ScheduleTimeout(t) => self.timer.schedule(t),
            ActionInner::RevealNonce {
                chain_id,
                branch,
                level,
                nonce,
            } => {
                let content = Contents::SeedNonceRevelation(SeedNonceRevelationOperation {
                    level,
                    nonce: SizedBytes(nonce.as_slice().try_into().unwrap()),
                });
                let mut bytes = branch.0.clone();
                content.bin_write(&mut bytes).unwrap();
                bytes.extend_from_slice(&[0; 64]);
                let op_hex = hex::encode(bytes);
                match self.client.inject_operation(&chain_id, op_hex, true) {
                    Ok(hash) => slog::info!(self.log, " .  inject nonce_reveal: {hash}"),
                    Err(err) => slog::error!(self.log, " .  {err}"),
                }
            }
            ActionInner::PreVote(chain_id, op) => {
                let (data, _) = self.crypto.sign(0x12, &chain_id, &op).unwrap();
                match self
                    .client
                    .inject_operation(&chain_id, hex::encode(data), false)
                {
                    Ok(hash) => slog::info!(self.log, " .  inject preendorsement: {hash}"),
                    Err(err) => slog::error!(self.log, " .  {err}"),
                }
            }
            ActionInner::Vote(chain_id, op) => {
                let (data, _) = self.crypto.sign(0x13, &chain_id, &op).unwrap();
                match self
                    .client
                    .inject_operation(&chain_id, hex::encode(data), false)
                {
                    Ok(hash) => slog::info!(self.log, " .  inject endorsement: {hash}"),
                    Err(err) => slog::error!(self.log, " .  {err}"),
                }
            }
            ActionInner::Propose {
                chain_id,
                proof_of_work_threshold,
                mut protocol_header,
                predecessor_hash,
                operations,
                timestamp,
                round,
            } => {
                let (_, signature) = self.crypto.sign(0x11, &chain_id, &protocol_header).unwrap();
                protocol_header.signature = signature;

                let (mut header, ops) = match self.client.preapply_block(
                    protocol_header,
                    predecessor_hash.clone(),
                    timestamp,
                    operations,
                ) {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(self.log, " .  {err}");
                        return;
                    }
                };

                header.signature.0 = vec![0x00; 64];
                let p = guess_proof_of_work(&header, proof_of_work_threshold);
                header.proof_of_work_nonce = SizedBytes(p);
                slog::info!(self.log, "{:?}", header);
                header.signature.0.clear();
                let (data, _) = self.crypto.sign(0x11, &chain_id, &header).unwrap();

                let valid_operations = ops
                    .iter()
                    .filter_map(|v| {
                        let applied = v.as_object()?.get("applied")?.clone();
                        serde_json::from_value(applied).ok()
                    })
                    .collect();

                match self
                    .client
                    .inject_block(hex::encode(data), valid_operations)
                {
                    Ok(hash) => slog::info!(
                        self.log,
                        " .  inject block: {}:{}, {hash}",
                        header.level,
                        round
                    ),
                    Err(err) => {
                        slog::error!(self.log, " .  {err}");
                        slog::error!(self.log, " .  {}", serde_json::to_string(&ops).unwrap());
                    }
                }
            }
        }
    }
}

impl TimeService for Services {}
