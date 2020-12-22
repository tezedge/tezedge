use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};
use tezos_messages::p2p::encoding::prelude::*;

use super::base_types::*;

type ChainId = UniString;

// GET /monitor/protocols
type ProtocolHash = UniString;

// GET /monitor/active_chains

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ChainStatus {
    Active {
        chain_id: ChainId,
        #[serde(skip_serializing_if = "Option::is_none")]
        test_protocol: Option<ProtocolHash>,
        #[serde(skip_serializing_if = "Option::is_none")]
        expiration_date: Option<TimeStamp>,
    },
    Stopping {
        stopping: ChainId,
    },
}

impl ChainStatus {
    /// Create basic chain status report without test_protocol and expiration_date
    pub fn basic<T: Into<UniString>>(chain_id: T) -> Self {
        Self::Active {
            chain_id: chain_id.into(),
            test_protocol: None,
            expiration_date: None,
        }
    }

    /// Create detailed chain status with all attributes
    pub fn detailed<C: Into<UniString>, H: Into<UniString>>(
        chain_id: C,
        test_protocol: H,
        expiration_date: TimeStamp,
    ) -> Self {
        Self::Active {
            chain_id: chain_id.into(),
            test_protocol: Some(test_protocol.into()),
            expiration_date: Some(expiration_date),
        }
    }

    /// Create chain status for a stopping chain
    pub fn stopping<T: Into<UniString>>(chain_id: T) -> Self {
        Self::Stopping {
            stopping: chain_id.into(),
        }
    }
}

pub type ActiveChains = Vec<ChainStatus>;

/// Bootstrap streaming info
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BootstrapInfo {
    block: String,
    timestamp: TimeStamp,
}

impl BootstrapInfo {
    pub fn new(block: &BlockHash, timestamp: TimeStamp) -> Self {
        Self {
            block: HashType::BlockHash.hash_to_b58check(block),
            timestamp,
        }
    }
}

// GET /monitor/heads/<chain_id>?(next_protocol=<Protocol_hash>)*

pub type OperationListHash = Vec<Vec<String>>;
pub type Fitness = String;
pub type ContextHash = UniString;

pub type ChainHead = BlockHeader;

// GET /monitor/valid_blocks?(protocol=<Protocol_hash>)*&(next_protocol=<Protocol_hash>)*&(chain=<chain_id>)*

// Monitor all blocks that are successfully validated by the node, disregarding whether they were selected as the new head or not.
// Optional query arguments :
//    protocol = <Protocol_hash>
//    next_protocol = <Protocol_hash>
//    chain = <chain_id>

// { "chain_id": $Chain_id,
//    "hash": $block_hash,
//    "level": integer ∈ [-2^31-2, 2^31+2],
//    "proto": integer ∈ [0, 255],
//    "predecessor": $block_hash,
//    "timestamp": $timestamp.protocol,
//    "validation_pass": integer ∈ [0, 255],
//    "operations_hash": $Operation_list_list_hash,
//    "fitness": $fitness,
//    "context": $Context_hash,
//    "protocol_data": /^[a-zA-Z0-9]+$/ }
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidBlocks {
    chain_id: ChainId,
    #[serde(flatten)]
    inner: ChainHead,
}

#[cfg(test)]
mod tests {
    use crate::encoding::test_helpers::*;

    use super::*;

    mod bootstrapped {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            let original = BootstrapInfo {
                block: "test".into(),
                timestamp: TimeStamp::Integral(10),
            };
            let encoded = serde_json::to_string(&original)?;
            let decoded = serde_json::from_str(&encoded)?;
            assert_eq!(original, decoded);
            Ok(())
        }

        #[test]
        fn encoded_custom() -> Result<(), serde_json::Error> {
            let ct = "test";
            let ts = 10;
            let original = BootstrapInfo {
                block: ct.into(),
                timestamp: TimeStamp::Integral(ts),
            };
            custom_encoded(
                original,
                &format!("{{\"block\":\"{}\",\"timestamp\":{}}}", ct, ts),
            )
        }

        #[test]
        fn decoded_custom() -> Result<(), serde_json::Error> {
            let ct = "test";
            let ts = 10;
            custom_decoded(
                &format!("{{\"block\":\"{}\",\"timestamp\":{}}}", ct, ts),
                BootstrapInfo {
                    block: ct.into(),
                    timestamp: TimeStamp::Integral(ts),
                },
            )
        }
    }

    mod active_chain {
        use super::*;

        #[test]
        fn encoded_equals_decoded() -> Result<(), serde_json::Error> {
            for original in &[
                ChainStatus::basic("test"),
                ChainStatus::detailed("test", "test", TimeStamp::Integral(10)),
                ChainStatus::stopping("test"),
            ] {
                let encoded = serde_json::to_string(original)?;
                let decoded: ChainStatus = serde_json::from_str(&encoded)?;
                assert_eq!(original, &decoded);
            }
            Ok(())
        }

        #[test]
        fn encoded_custom_basic() -> Result<(), serde_json::Error> {
            let content = "test";
            let original = ChainStatus::basic(content);
            custom_encoded(original, &format!("{{\"chain_id\":\"{}\"}}", content))
        }

        #[test]
        fn encoded_custom_detailed() -> Result<(), serde_json::Error> {
            let ct = "test";
            let ts = 10;
            let original = ChainStatus::detailed(ct, ct, TimeStamp::Integral(ts));
            custom_encoded(
                original,
                &format!(
                    "{{\"chain_id\":\"{}\",\"test_protocol\":\"{}\",\"expiration_date\":{}}}",
                    ct, ct, ts
                ),
            )
        }

        #[test]
        fn encoded_custom_stopping() -> Result<(), serde_json::Error> {
            let ct = "test";
            let original = ChainStatus::stopping(ct);
            custom_encoded(original, &format!("{{\"stopping\":\"{}\"}}", ct))
        }

        #[test]
        fn decoded_custom_basic() -> Result<(), serde_json::Error> {
            let content = "test";
            custom_decoded(
                &format!("{{\"chain_id\":\"{}\"}}", content),
                ChainStatus::basic(content),
            )
        }

        #[test]
        fn decoded_custom_detailed() -> Result<(), serde_json::Error> {
            let ct = "test";
            let ts = 10;
            custom_decoded(
                &format!(
                    "{{\"chain_id\":\"{}\",\"test_protocol\":\"{}\",\"expiration_date\":{}}}",
                    ct, ct, ts
                ),
                ChainStatus::detailed(ct, ct, TimeStamp::Integral(ts)),
            )
        }

        #[test]
        fn decoded_custom_stopping() -> Result<(), serde_json::Error> {
            let ct = "test";
            custom_decoded(
                &format!("{{\"stopping\":\"{}\"}}", ct),
                ChainStatus::stopping(ct),
            )
        }
    }
}
