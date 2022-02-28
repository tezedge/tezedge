// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, CryptoboxPublicKeyHash};
use tezos_api::ffi::{ApplyBlockExecutionTimestamps, ApplyBlockResponse};
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::block_header::Level;

fn ocaml_time_normalize(ocaml_time: f64) -> u64 {
    (ocaml_time * 1_000_000_000.0) as u64
}

pub type BlocksApplyStats = HashMap<BlockHash, BlockApplyStats>;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlockApplyStats {
    pub level: Level,
    pub block_timestamp: i64,
    pub validation_pass: u8,

    pub receive_timestamp: u64,

    pub injected: Option<u64>,
    pub baker: Option<SignaturePublicKey>,
    pub priority: Option<u16>,

    pub precheck_start: Option<u64>,
    pub precheck_end: Option<u64>,

    pub download_block_header_start: Option<u64>,
    pub download_block_header_end: Option<u64>,

    pub download_block_operations_start: Option<u64>,
    pub download_block_operations_end: Option<u64>,

    pub load_data_start: Option<u64>,
    pub load_data_end: Option<u64>,

    pub apply_block_start: Option<u64>,
    pub apply_block_end: Option<u64>,
    pub apply_block_stats: Option<ApplyBlockProtocolStats>,

    pub store_result_start: Option<u64>,
    pub store_result_end: Option<u64>,

    pub head_send_start: Option<u64>,
    pub head_send_end: Option<u64>,

    pub peers: HashMap<SocketAddr, BlockPeerStats>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlockPeerStats {
    pub node_id: Option<CryptoboxPublicKeyHash>,
    pub head_recv: Vec<u64>,
    pub head_send_start: Vec<u64>,
    pub head_send_end: Vec<u64>,
    pub get_ops_recv: Vec<(u64, i8)>,
    pub ops_send_start: Vec<(u64, i8)>,
    pub ops_send_end: Vec<(u64, i8)>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ApplyBlockProtocolStats {
    pub apply_start: u64,
    pub operations_decoding_start: u64,
    pub operations_decoding_end: u64,
    pub operations_application: Vec<Vec<(u64, u64)>>,
    pub operations_metadata_encoding_start: u64,
    pub operations_metadata_encoding_end: u64,
    pub begin_application_start: u64,
    pub begin_application_end: u64,
    pub finalize_block_start: u64,
    pub finalize_block_end: u64,
    pub collect_new_rolls_owner_snapshots_start: u64,
    pub collect_new_rolls_owner_snapshots_end: u64,
    pub commit_start: u64,
    pub commit_end: u64,
    pub apply_end: u64,
}

impl From<&ApplyBlockExecutionTimestamps> for ApplyBlockProtocolStats {
    fn from(v: &ApplyBlockExecutionTimestamps) -> Self {
        Self {
            apply_start: ocaml_time_normalize(v.apply_start_t),
            operations_decoding_start: ocaml_time_normalize(v.operations_decoding_start_t),
            operations_decoding_end: ocaml_time_normalize(v.operations_decoding_end_t),
            operations_application: v
                .operations_application_timestamps
                .iter()
                .map(|l| {
                    l.iter()
                        .map(|(start, end)| {
                            (ocaml_time_normalize(*start), ocaml_time_normalize(*end))
                        })
                        .collect()
                })
                .collect(),
            operations_metadata_encoding_start: ocaml_time_normalize(
                v.operations_metadata_encoding_start_t,
            ),
            operations_metadata_encoding_end: ocaml_time_normalize(
                v.operations_metadata_encoding_end_t,
            ),
            begin_application_start: ocaml_time_normalize(v.begin_application_start_t),
            begin_application_end: ocaml_time_normalize(v.begin_application_end_t),
            finalize_block_start: ocaml_time_normalize(v.finalize_block_start_t),
            finalize_block_end: ocaml_time_normalize(v.finalize_block_end_t),
            collect_new_rolls_owner_snapshots_start: ocaml_time_normalize(
                v.collect_new_rolls_owner_snapshots_start_t,
            ),
            collect_new_rolls_owner_snapshots_end: ocaml_time_normalize(
                v.collect_new_rolls_owner_snapshots_end_t,
            ),
            commit_start: ocaml_time_normalize(v.commit_start_t),
            commit_end: ocaml_time_normalize(v.commit_end_t),
            apply_end: ocaml_time_normalize(v.apply_end_t),
        }
    }
}

#[derive(Debug, Default)]
pub struct StatisticsService {
    blocks_apply: BlocksApplyStats,
    levels: VecDeque<(Level, Vec<BlockHash>)>,
}

impl StatisticsService {
    pub fn block_stats_get_all(&self) -> &BlocksApplyStats {
        &self.blocks_apply
    }

    pub fn block_stats_get_by_level(&self, level: Level) -> Vec<(BlockHash, BlockApplyStats)> {
        match self.levels.binary_search_by_key(&level, |(l, _)| *l) {
            Ok(idx) => self.levels[idx]
                .1
                .iter()
                .filter_map(|h| {
                    self.blocks_apply
                        .get(h)
                        .and_then(|s| Some((h.clone(), s.clone())))
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn add_block_level(
        levels: &mut VecDeque<(Level, Vec<BlockHash>)>,
        block_hash: BlockHash,
        level: Level,
    ) {
        match levels.back_mut() {
            Some((l, hs)) if l == &level => hs.push(block_hash.clone()),
            Some((l, _)) if *l < level => levels.push_back((level, vec![block_hash.clone()])),
            None => levels.push_back((level, vec![block_hash.clone()])),
            _ => match levels.binary_search_by_key(&level, |(l, _)| *l) {
                Ok(idx) => levels[idx].1.push(block_hash.clone()),
                Err(idx) => levels.insert(idx, (level, vec![block_hash.clone()])),
            },
        }
        levels.drain(0..(levels.len().saturating_sub(120)));
    }

    pub fn block_new(
        &mut self,
        block_hash: BlockHash,
        level: Level,
        block_timestamp: i64,
        validation_pass: u8,
        receive_timestamp: u64,
        peer: Option<SocketAddr>,
        node_id: Option<CryptoboxPublicKeyHash>,
        injected_timestamp: Option<u64>,
    ) {
        let levels = &mut self.levels;
        let stats = self
            .blocks_apply
            .entry(block_hash.clone())
            .or_insert_with(|| {
                Self::add_block_level(levels, block_hash.clone(), level);
                BlockApplyStats {
                    level,
                    block_timestamp,
                    validation_pass,
                    receive_timestamp,
                    injected: injected_timestamp,
                    ..BlockApplyStats::default()
                }
            });
        if let Some(peer) = peer {
            stats
                .peers
                .entry(peer)
                .or_insert_with(|| BlockPeerStats {
                    node_id,
                    ..BlockPeerStats::default()
                })
                .head_recv
                .push(receive_timestamp);
        }
    }

    pub fn block_send_start(
        &mut self,
        block_hash: &BlockHash,
        peer: SocketAddr,
        node_id: Option<&CryptoboxPublicKeyHash>,
        time: u64,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.peers
                .entry(peer)
                .or_insert_with(|| BlockPeerStats {
                    node_id: node_id.cloned(),
                    ..BlockPeerStats::default()
                })
                .head_send_start
                .push(time);
            v.head_send_start.get_or_insert(time)
        });
    }

    pub fn block_send_end(
        &mut self,
        block_hash: &BlockHash,
        peer: SocketAddr,
        node_id: Option<&CryptoboxPublicKeyHash>,
        time: u64,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.peers
                .entry(peer)
                .or_insert_with(|| BlockPeerStats {
                    node_id: node_id.cloned(),
                    ..BlockPeerStats::default()
                })
                .head_send_end
                .push(time);
            v.head_send_end = Some(time);
        });
    }

    pub fn block_precheck_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.precheck_start = Some(time);
        });
    }

    pub fn block_precheck_end(
        &mut self,
        block_hash: &BlockHash,
        baker: SignaturePublicKey,
        priority: u16,
        time: u64,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.precheck_end = Some(time);
            v.baker = Some(baker);
            v.priority = Some(priority);
        });
    }

    pub fn block_header_download_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .filter(|v| v.load_data_start.is_none())
            .map(|v| v.download_block_header_start.get_or_insert(time));
    }

    pub fn block_header_download_end(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .filter(|v| v.load_data_start.is_none())
            .map(|v| v.download_block_header_end.get_or_insert(time));
    }

    pub fn block_operations_download_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .filter(|v| v.load_data_start.is_none())
            .map(|v| v.download_block_operations_start.get_or_insert(time));
    }

    pub fn block_operations_download_end(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .filter(|v| v.load_data_start.is_none())
            .map(|v| v.download_block_operations_end = Some(time));
    }

    /// Started loading block data from storage for block application.
    pub fn block_load_data_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .map(|v| v.load_data_start = Some(time));
    }

    /// Finished loading block data from storage for block application.
    pub fn block_load_data_end(&mut self, block_hash: &BlockHash, block_level: Level, time: u64) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.level = block_level;
            v.load_data_end = Some(time);
        });
    }

    pub fn block_apply_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .map(|v| v.apply_block_start = Some(time));
    }

    pub fn block_apply_end(
        &mut self,
        block_hash: &BlockHash,
        time: u64,
        result: &ApplyBlockResponse,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.apply_block_stats = Some((&result.execution_timestamps).into());
            v.apply_block_end = Some(time)
        });
    }

    pub fn block_store_result_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .map(|v| v.store_result_start = Some(time));
    }

    pub fn block_store_result_end(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks_apply
            .get_mut(block_hash)
            .map(|v| v.store_result_end = Some(time));
    }

    pub fn block_get_operations_recv(
        &mut self,
        block_hash: &BlockHash,
        time: u64,
        address: SocketAddr,
        node_id: Option<&CryptoboxPublicKeyHash>,
        validation_pass: i8,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.peers
                .entry(address)
                .or_insert_with(|| BlockPeerStats {
                    node_id: node_id.cloned(),
                    ..BlockPeerStats::default()
                })
                .get_ops_recv
                .push((time, validation_pass));
        });
    }

    pub fn block_operations_send_start(
        &mut self,
        block_hash: &BlockHash,
        time: u64,
        address: SocketAddr,
        node_id: Option<&CryptoboxPublicKeyHash>,
        validation_pass: i8,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.peers
                .entry(address)
                .or_insert_with(|| BlockPeerStats {
                    node_id: node_id.cloned(),
                    ..BlockPeerStats::default()
                })
                .ops_send_start
                .push((time, validation_pass));
        });
    }

    pub fn block_operations_send_end(
        &mut self,
        block_hash: &BlockHash,
        time: u64,
        address: SocketAddr,
        node_id: Option<&CryptoboxPublicKeyHash>,
        validation_pass: i8,
    ) {
        self.blocks_apply.get_mut(block_hash).map(|v| {
            v.peers
                .entry(address)
                .or_insert_with(|| BlockPeerStats {
                    node_id: node_id.cloned(),
                    ..BlockPeerStats::default()
                })
                .ops_send_end
                .push((time, validation_pass));
        });
    }

    pub fn block_injected(&mut self, block_hash: &BlockHash, time: u64) {
        if let Some(v) = self.blocks_apply.get_mut(block_hash) {
            v.injected = Some(time)
        }
    }
}
