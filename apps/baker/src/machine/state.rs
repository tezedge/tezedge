// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ContractTz1Hash, OperationHash, ProtocolHash, Signature};
use tenderbake as tb;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct Nonce(pub Vec<u8>);

impl Serialize for Nonce {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            hex::encode(&self.0).serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Nonce {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            hex::decode(String::deserialize(deserializer)?)
                .map_err(serde::de::Error::custom)
                .map(Nonce)
        } else {
            Vec::deserialize(deserializer).map(Nonce)
        }
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct CycleNonces {
    // immutable config
    pub blocks_per_commitment: u32,
    pub blocks_per_cycle: u32,
    pub nonce_length: usize,
    // this cycle
    pub cycle: u32,
    // map from nonce into position in cycle
    // previous cycle: (cycle - 1)
    pub previous: BTreeMap<Nonce, u32>,
    // this cycle
    pub this: BTreeMap<Nonce, u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SlotsInfo {
    pub committee_size: u32,
    pub ours: Vec<ContractTz1Hash>,
    pub level: i32,
    pub delegates: BTreeMap<i32, BTreeMap<ContractTz1Hash, Vec<u16>>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OperationSimple {
    pub branch: BlockHash,
    pub contents: Vec<serde_json::Value>,
    #[serde(default)]
    pub signature: Option<Signature>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<OperationHash>,
    #[serde(default)]
    pub protocol: Option<ProtocolHash>,
}

#[allow(dead_code)]
pub struct State {
    pub nonces: CycleNonces,
    pub retry_monitor_operations: usize,
    pub tb_delegate: tb::Config<tb::TimingLinearGrow, SlotsInfo>,
    pub tb_state: tb::Machine<ContractTz1Hash, OperationSimple, 200>,
}
