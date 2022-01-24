// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap},
    net::SocketAddr,
};

use crate::ActionId;
use crypto::hash::{BlockHash, CryptoboxPublicKeyHash};
use tezos_messages::{
    base::signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash},
    p2p::encoding::block_header::Level,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CurrentHeadStats {
    pub level_stats: HashMap<Level, CurrentHeadLevelStats>,
    pub pending_messages: HashMap<SocketAddr, (Level, BlockHash)>,
    pub last_pruned: Option<ActionId>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentHeadLevelStats {
    pub first_address: SocketAddr,
    pub first_action: ActionId,
    pub last_action: ActionId,
    pub head_stats: BTreeMap<BlockHash, CurrentHeadData>,
    pub peer_stats: CurrentHeadPeerStats,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, getset::Getters)]
#[getset(get = "pub(super)")]
pub struct CurrentHeadData {
    pub baker: SignaturePublicKey,
    pub priority: u16,
    pub prechecked_time: u64,
}

pub type CurrentHeadPeerStats = HashMap<SocketAddr, PeerCurrentHeadData>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerCurrentHeadData {
    pub node_id: Option<CryptoboxPublicKeyHash>,
    pub hash: BlockHash,
    pub baker: Option<SignaturePublicKeyHash>,
    pub times: HashMap<String, u64>,
}
