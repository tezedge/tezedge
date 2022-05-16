// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, str};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockPayloadHash, OperationHash, ProtocolHash, Signature};
use tezos_messages::protocol::proto_012::operation::EndorsementOperation;

use super::operation_mutator::OperationContentMutator;

#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct OperationSimple {
    pub branch: BlockHash,
    #[cfg_attr(feature = "fuzzing", field_mutator(OperationContentMutator))]
    pub contents: Vec<serde_json::Value>,
    #[serde(default)]
    pub signature: Option<Signature>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<OperationHash>,
    #[serde(default)]
    pub protocol: Option<ProtocolHash>,
}

impl OperationSimple {
    #[cfg(test)]
    pub fn new(branch: &BlockHash, content: &str) -> Self {
        OperationSimple {
            branch: branch.clone(),
            contents: vec![serde_json::from_str(content).unwrap()],
            signature: None,
            hash: None,
            protocol: None,
        }
    }

    #[cfg(test)]
    pub fn preendorsement(
        branch: &BlockHash,
        payload_hash: &BlockPayloadHash,
        level: i32,
        round: i32,
        slot: u16,
    ) -> Self {
        Self::new(branch, &format!("{{\"block_payload_hash\":\"{payload_hash}\",\"kind\":\"preendorsement\",\"level\":{level},\"round\":{round},\"slot\":{slot}}}"))
    }
}

pub enum OperationKind {
    Preendorsement(EndorsementOperation),
    Endorsement(EndorsementOperation),
    Votes,
    Anonymous,
    Managers,
}

impl fmt::Debug for OperationSimple {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Operation")
            .field("branch", &self.branch)
            .field("hash", &self.hash)
            .field("protocol", &self.protocol)
            .field("contents", &serde_json::to_string(&self.contents))
            .field("signature", &self.signature)
            .finish()
    }
}

impl OperationSimple {
    pub fn kind(&self) -> Option<OperationKind> {
        fn op_kind(c: &serde_json::Value) -> Option<&str> {
            c.as_object()?.get("kind")?.as_str()
        }

        let c = self.contents.first()?.clone();

        match op_kind(&c)? {
            "preendorsement" => {
                let mut c = c;
                let c_obj = c.as_object_mut()?;
                c_obj.remove("kind");
                c_obj.remove("metadata");
                serde_json::from_value(c)
                    .ok()
                    .map(OperationKind::Preendorsement)
            }
            "endorsement" => {
                let mut c = c;
                let c_obj = c.as_object_mut()?;
                c_obj.remove("kind");
                c_obj.remove("metadata");
                serde_json::from_value(c)
                    .ok()
                    .map(OperationKind::Endorsement)
            }
            "failing_noop" => None,
            "proposals" | "ballot" => Some(OperationKind::Votes),
            "seed_nonce_revelation"
            | "double_preendorsement_evidence"
            | "double_endorsement_evidence"
            | "double_baking_evidence"
            | "activate_account" => Some(OperationKind::Anonymous),
            _ => Some(OperationKind::Managers),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Block {
    pub hash: BlockHash,
    pub level: i32,
    pub predecessor: BlockHash,
    pub timestamp: u64,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    pub round: i32,
    pub transition: bool,
}

/// Adapter for serialize and deserialize
#[derive(Clone, Debug)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Slots(pub Vec<u16>);

impl Serialize for Slots {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        format!("{:?}", self.0).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Slots {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        let s = String::deserialize(deserializer)?;
        let s = s.trim_start_matches('[').trim_end_matches(']');
        let mut slots = Slots(vec![]);
        for slot_str in s.split(',') {
            let slot = slot_str.trim().parse().map_err(D::Error::custom)?;
            slots.0.push(slot);
        }
        Ok(slots)
    }
}
