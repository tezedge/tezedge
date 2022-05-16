// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockPayloadHash;
use fuzzcheck::{mutators::map::MapMutator, DefaultMutator, MutatorWrapper};

#[derive(Clone)]
#[cfg_attr(feature = "fuzzing", derive(DefaultMutator))]
pub enum OperationContentInner {
    Preendorsement {
        payload_hash: BlockPayloadHash,
        level: i32,
        round: i32,
        slot: u16,
    },
    Endorsement {
        payload_hash: BlockPayloadHash,
        level: i32,
        round: i32,
        slot: u16,
    },
    Other,
}

pub struct OperationContentMutator(OperationContentMutatorInner);

impl MutatorWrapper for OperationContentMutator {
    type Wrapped = OperationContentMutatorInner;

    #[no_coverage]
    fn wrapped_mutator(&self) -> &Self::Wrapped {
        &self.0
    }
}

impl Default for OperationContentMutator {
    #[no_coverage]
    fn default() -> Self {
        OperationContentMutator(MapMutator::new(
            OperationContentInner::default_mutator(),
            parse,
            map,
            complexity,
        ))
    }
}

type OperationContentMutatorInner = MapMutator<
    OperationContentInner,
    Vec<serde_json::Value>,
    <OperationContentInner as DefaultMutator>::Mutator,
    fn(&Vec<serde_json::Value>) -> Option<OperationContentInner>,
    fn(&OperationContentInner) -> Vec<serde_json::Value>,
    fn(&Vec<serde_json::Value>, f64) -> f64,
>;

#[no_coverage]
fn parse(v: &Vec<serde_json::Value>) -> Option<OperationContentInner> {
    let obj = v.first()?.as_object()?;
    let kind = obj.get("kind")?.as_str()?;

    match kind {
        "preendorsement" => Some(OperationContentInner::Preendorsement {
            payload_hash: BlockPayloadHash::from_base58_check(
                obj.get("block_payload_hash")?.as_str()?,
            )
            .unwrap(),
            level: obj.get("level")?.as_i64()? as i32,
            round: obj.get("round")?.as_i64()? as i32,
            slot: obj.get("slot")?.as_i64()? as u16,
        }),
        "endorsement" => Some(OperationContentInner::Endorsement {
            payload_hash: BlockPayloadHash::from_base58_check(
                obj.get("block_payload_hash")?.as_str()?,
            )
            .unwrap(),
            level: obj.get("level")?.as_i64()? as i32,
            round: obj.get("round")?.as_i64()? as i32,
            slot: obj.get("slot")?.as_i64()? as u16,
        }),
        _ => Some(OperationContentInner::Other),
    }
}

#[cfg(test)]
#[test]
fn foo() {
    map(&OperationContentInner::Other);
}

#[no_coverage]
fn map(v: &OperationContentInner) -> Vec<serde_json::Value> {
    let json = match v {
        OperationContentInner::Preendorsement {
            payload_hash,
            level,
            round,
            slot,
        } => {
            format!(
                "\
                {{\
                    \"block_payload_hash\":\"{payload_hash}\",\
                    \"kind\":\"preendorsement\",\
                    \"level\":{level},\
                    \"round\":{round},\
                    \"slot\":{slot}\
                }}\
            "
            )
        }
        OperationContentInner::Endorsement {
            payload_hash,
            level,
            round,
            slot,
        } => {
            format!(
                "\
                {{\
                    \"block_payload_hash\":\"{payload_hash}\",\
                    \"kind\":\"endorsement\",\
                    \"level\":{level},\
                    \"round\":{round},\
                    \"slot\":{slot}\
                }}\
            "
            )
        }
        OperationContentInner::Other => "{\"kind\":\"falling_noop\"}".to_string(),
    };
    vec![serde_json::from_str(&json).unwrap()]
}

#[no_coverage]
fn complexity(_t: &Vec<serde_json::Value>, cplx: f64) -> f64 {
    cplx
}
