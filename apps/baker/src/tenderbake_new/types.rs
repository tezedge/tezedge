// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, ops::AddAssign};

use serde::{Deserialize, Serialize};

#[cfg(not(feature = "testing-mock"))]
use crypto::hash::Signature;

#[cfg(not(feature = "testing-mock"))]
use super::hash::ProtocolHash;

use super::{
    hash::{BlockHash, BlockPayloadHash, ContractTz1Hash, OperationHash},
    timestamp::{TimeHeader, Timestamp},
};

pub enum Event {
    Proposal(Block, Timestamp),
    Preendorsed { vote: Vote },
    Endorsed { vote: Vote, now: Timestamp },
    Operation(OperationSimple),
    Timeout,
}

pub enum Action {
    ScheduleTimeout(Timestamp),
    Preendorse(BlockHash, BlockId),
    Endorse(BlockHash, BlockId),
    Propose(Block, ContractTz1Hash, bool),
    // log actions
    Proposal {
        level: i32,
        round: i32,
        timestamp: Timestamp,
    },
    UnexpectedLevel {
        current: i32,
    },
    NoPredecessor,
    Predecessor {
        round: i32,
        timestamp: Timestamp,
    },
    TwoTransitionsInRow,
    UnexpectedRoundFromFuture {
        current: i32,
        block_round: i32,
    },
    HavePreCertificate {
        payload_round: i32,
    },
    HaveCertificate,
    NoSwitchBranch,
    SwitchBranch,
    UnexpectedRoundBounded {
        last: i32,
        current: i32,
    },
    WillBakeThisLevel {
        round: i32,
        timestamp: Timestamp,
    },
    WillBakeNextLevel {
        round: i32,
        timestamp: Timestamp,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warn,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlockId {
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

pub struct Block {
    pub pred_hash: BlockHash,
    pub hash: BlockHash,
    pub level: i32,
    pub time_header: TimeHeader<false>,
    pub payload: Option<Payload>,
}

#[derive(Clone)]
pub struct Payload {
    pub hash: BlockPayloadHash,
    pub payload_round: i32,
    pub pre_cer: Option<PreCertificate>,
    pub cer: Option<Certificate>,
    pub operations: Vec<OperationSimple>,
}

// TODO: rename
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WithH<T> {
    pub branch: BlockHash,
    #[cfg(not(feature = "testing-mock"))]
    #[serde(default)]
    pub signature: Option<Signature>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<OperationHash>,
    #[cfg(not(feature = "testing-mock"))]
    #[serde(default)]
    pub protocol: Option<ProtocolHash>,
    pub contents: T,
}

impl<T> WithH<T> {
    #[cfg(feature = "testing-mock")]
    pub fn new(branch: BlockHash, contents: T) -> Self {
        WithH {
            branch,
            hash: None,
            contents,
        }
    }

    pub fn map<F, V>(self, f: F) -> WithH<V>
    where
        F: Fn(T) -> V,
    {
        WithH {
            branch: self.branch,
            #[cfg(not(feature = "testing-mock"))]
            signature: self.signature,
            hash: self.hash,
            #[cfg(not(feature = "testing-mock"))]
            protocol: self.protocol,
            contents: f(self.contents),
        }
    }
}

#[derive(Clone, Debug)]
pub enum OperationSimple {
    Preendorsement(WithH<ConsensusBody>),
    Endorsement(WithH<ConsensusBody>),
    #[cfg(not(feature = "testing-mock"))]
    Votes(WithH<Vec<serde_json::Value>>),
    #[cfg(not(feature = "testing-mock"))]
    Anonymous(WithH<Vec<serde_json::Value>>),
    #[cfg(not(feature = "testing-mock"))]
    Managers(WithH<Vec<serde_json::Value>>),
}

impl OperationSimple {
    pub fn branch(&self) -> &BlockHash {
        match self {
            OperationSimple::Preendorsement(op) => &op.branch,
            OperationSimple::Endorsement(op) => &op.branch,
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Votes(op) => &op.branch,
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Anonymous(op) => &op.branch,
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Managers(op) => &op.branch,
        }
    }

    pub fn hash(&self) -> Option<&OperationHash> {
        match self {
            OperationSimple::Preendorsement(op) => op.hash.as_ref(),
            OperationSimple::Endorsement(op) => op.hash.as_ref(),
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Votes(op) => op.hash.as_ref(),
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Anonymous(op) => op.hash.as_ref(),
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Managers(op) => op.hash.as_ref(),
        }
    }

    pub fn strip_signature(&mut self) {
        #[cfg(not(feature = "testing-mock"))]
        match self {
            OperationSimple::Anonymous(op) => {
                op.signature = Some(Signature(vec![0x55; 32]));
            }
            _ => (),
        }
    }

    pub fn strip(&mut self) {
        match self {
            OperationSimple::Preendorsement(op) => {
                op.hash = None;
            }
            OperationSimple::Endorsement(op) => {
                op.hash = None;
            }
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Votes(op)
            | OperationSimple::Anonymous(op)
            | OperationSimple::Managers(op) => {
                op.hash = None;
                for content in &mut op.contents {
                    if let Some(content_obj) = content.as_object_mut() {
                        content_obj.remove("metadata");
                    }
                }
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConsensusBody {
    pub slot: u16,
    pub level: i32,
    pub round: i32,
    pub block_payload_hash: BlockPayloadHash,
}

impl ConsensusBody {
    fn from_json(v: Vec<serde_json::Value>) -> Option<Self> {
        let f = v.first()?.as_object()?;

        Some(ConsensusBody {
            slot: f.get("slot")?.as_i64()? as u16,
            level: f.get("level")?.as_i64()? as i32,
            round: f.get("round")?.as_i64()? as i32,
            block_payload_hash: {
                let hash = f.get("block_payload_hash")?.as_str()?;
                BlockPayloadHash::from_base58_check(hash).ok()?
            },
        })
    }

    pub fn into_json(self, kind: &str) -> Vec<serde_json::Value> {
        vec![serde_json::Value::Object({
            let mut m = serde_json::Map::new();
            m.insert(
                "kind".to_string(),
                serde_json::Value::String(kind.to_string()),
            );
            m.insert(
                "slot".to_string(),
                serde_json::Value::Number(self.slot.into()),
            );
            m.insert(
                "level".to_string(),
                serde_json::Value::Number(self.level.into()),
            );
            m.insert(
                "round".to_string(),
                serde_json::Value::Number(self.round.into()),
            );
            m.insert(
                "block_payload_hash".to_string(),
                serde_json::Value::String(self.block_payload_hash.to_string()),
            );
            m
        })]
    }

    pub fn block_id(self) -> BlockId {
        BlockId {
            level: self.level,
            round: self.round,
            block_payload_hash: self.block_payload_hash,
        }
    }

    pub fn new(
        BlockId {
            level,
            round,
            block_payload_hash,
        }: BlockId,
        slot: u16,
    ) -> Self {
        ConsensusBody {
            slot,
            level,
            round,
            block_payload_hash,
        }
    }
}

impl Serialize for OperationSimple {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OperationSimple::Preendorsement(v) => v
                .clone()
                .map(|s| s.into_json("preendorsement"))
                .serialize(serializer),
            OperationSimple::Endorsement(v) => v
                .clone()
                .map(|s| s.into_json("endorsement"))
                .serialize(serializer),
            #[cfg(not(feature = "testing-mock"))]
            OperationSimple::Votes(v)
            | OperationSimple::Anonymous(v)
            | OperationSimple::Managers(v) => v.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for OperationSimple {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let op = WithH::<Vec<serde_json::Value>>::deserialize(deserializer)?;
        let kind = op
            .contents
            .first()
            .and_then(|v| v.as_object())
            .and_then(|obj| obj.get("kind"))
            .and_then(|kind| kind.as_str());
        let kind = match kind {
            Some(v) => v,
            None => return Err(serde::de::Error::custom("unknown operation kind")),
        };
        match kind {
            "preendorsement" => {
                let contents = ConsensusBody::from_json(op.contents)
                    .ok_or(serde::de::Error::custom("invalid operation json"))?;
                Ok(OperationSimple::Preendorsement(WithH {
                    branch: op.branch,
                    #[cfg(not(feature = "testing-mock"))]
                    signature: op.signature,
                    hash: op.hash,
                    #[cfg(not(feature = "testing-mock"))]
                    protocol: op.protocol,
                    contents,
                }))
            }
            "endorsement" => {
                let contents = ConsensusBody::from_json(op.contents)
                    .ok_or(serde::de::Error::custom("invalid operation json"))?;
                Ok(OperationSimple::Endorsement(WithH {
                    branch: op.branch,
                    #[cfg(not(feature = "testing-mock"))]
                    signature: op.signature,
                    hash: op.hash,
                    #[cfg(not(feature = "testing-mock"))]
                    protocol: op.protocol,
                    contents,
                }))
            }
            "failing_noop" => Err(serde::de::Error::custom("failing operation")),
            #[cfg(not(feature = "testing-mock"))]
            "proposals" | "ballot" => Ok(OperationSimple::Votes(op)),
            #[cfg(not(feature = "testing-mock"))]
            "seed_nonce_revelation"
            | "double_preendorsement_evidence"
            | "double_endorsement_evidence"
            | "double_baking_evidence"
            | "activate_account" => Ok(OperationSimple::Anonymous(op)),
            #[cfg(not(feature = "testing-mock"))]
            _ => Ok(OperationSimple::Managers(op)),
            #[cfg(feature = "testing-mock")]
            _ => Err(serde::de::Error::custom("deserialization not supported")),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Vote {
    pub id: ContractTz1Hash,
    pub power: u32,
    pub op: WithH<BlockId>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Votes {
    pub ids: BTreeMap<ContractTz1Hash, WithH<BlockId>>,
    pub power: u32,
}

impl AddAssign<Vote> for Votes {
    fn add_assign(&mut self, vote: Vote) {
        self.ids.insert(vote.id, vote.op);
        self.power += vote.power;
    }
}

impl FromIterator<Vote> for Votes {
    fn from_iter<T: IntoIterator<Item = Vote>>(iter: T) -> Self {
        let mut s = Votes::default();
        for vote in iter {
            s += vote;
        }
        s
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PreCertificate {
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    pub votes: Votes,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub votes: Votes,
}
