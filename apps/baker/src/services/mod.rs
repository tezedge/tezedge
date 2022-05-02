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
use tenderbake as tb;
use tezos_encoding::{enc::BinWriter, types::SizedBytes};
use tezos_messages::protocol::{
    proto_005_2::operation::SeedNonceRevelationOperation, proto_012::operation::Contents,
};

use crate::proof_of_work::guess_proof_of_work;

pub struct Services {
    pub client: client::RpcClient,
    pub crypto: key::CryptoService,
    log: slog::Logger,
    timer: timer::Timer,
    sender: mpsc::Sender<Result<event::Event, client::RpcError>>,
}

pub struct EventWithTime {
    pub event: Result<event::Event, client::RpcError>,
    pub now: tenderbake::Timestamp,
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
                EventWithTime { now, event }
            }),
        )
    }

    pub fn execute(&mut self, action: event::Action) {
        // TODO:
        match action {
            event::Action::Idle => drop(self.sender.send(Ok(event::Event::Idle))),
            event::Action::GetSlots { level } => match self.client.validators(level) {
                Ok(delegates) => {
                    let _ = self
                        .sender
                        .send(Ok(event::Event::Slots { level, delegates }));
                }
                Err(err) => drop(self.sender.send(Err(err))),
            },
            event::Action::GetOperationsForBlock { block_hash } => {
                match self.client.get_operations_for_block(&block_hash) {
                    Ok(operations) => {
                        let _ = self.sender.send(Ok(event::Event::OperationsForBlock {
                            block_hash,
                            operations,
                        }));
                    }
                    Err(err) => drop(self.sender.send(Err(err))),
                }
            }
            event::Action::GetLiveBlocks { block_hash } => {
                match self.client.get_live_blocks(&block_hash) {
                    Ok(live_blocks) => {
                        let _ = self.sender.send(Ok(event::Event::LiveBlocks {
                            block_hash,
                            live_blocks,
                        }));
                    }
                    Err(err) => drop(self.sender.send(Err(err))),
                }
            }
            event::Action::LogError(error) => slog::error!(self.log, " .  {error}"),
            event::Action::LogWarning(warn) => slog::warn!(self.log, " .  {warn}"),
            event::Action::LogTb(record) => match record.level() {
                tb::LogLevel::Info => slog::info!(self.log, "{record}"),
                tb::LogLevel::Warn => slog::warn!(self.log, "{record}"),
            },
            event::Action::MonitorOperations => {
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
            event::Action::ScheduleTimeout(t) => self.timer.schedule(t),
            event::Action::PreVote(chain_id, op) => {
                let (data, _) = self.crypto.sign(0x12, &chain_id, &op).unwrap();
                match self
                    .client
                    .inject_operation(&chain_id, hex::encode(data), false)
                {
                    Ok(hash) => slog::info!(self.log, " .  inject preendorsement: {hash}"),
                    Err(err) => slog::error!(self.log, " .  {err}"),
                }
            }
            event::Action::Vote(chain_id, op) => {
                let (data, _) = self.crypto.sign(0x13, &chain_id, &op).unwrap();
                match self
                    .client
                    .inject_operation(&chain_id, hex::encode(data), false)
                {
                    Ok(hash) => slog::info!(self.log, " .  inject endorsement: {hash}"),
                    Err(err) => slog::error!(self.log, " .  {err}"),
                }
            }
            event::Action::Propose {
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
            event::Action::RevealNonce {
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
        }
    }
}
