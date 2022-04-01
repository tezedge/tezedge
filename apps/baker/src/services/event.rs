// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, str};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, BlockPayloadHash, OperationHash, ProtocolHash, Signature};
use tezos_messages::protocol::proto_012::operation::EndorsementOperation;

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

pub struct Block {
    pub hash: BlockHash,
    pub level: i32,
    pub predecessor: BlockHash,
    pub timestamp: u64,
    pub fitness: Vec<String>,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    pub round: i32,

    pub transition: bool,
    pub operations: Vec<Vec<OperationSimple>>,
}

pub enum Event {
    Block(Block),
    Operations(Vec<OperationSimple>),
    Tick,
}
