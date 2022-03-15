// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

//! This crate provides definitions of tezos messages.

use getset::Getters;
use p2p::encoding::fitness::Fitness;
use serde::{Deserialize, Serialize};
use tezos_encoding::{
    enc::BinWriter,
    encoding::{Encoding, HasEncoding},
    nom::NomReader,
};
use time::{error::ComponentRange, format_description::well_known::Rfc3339, OffsetDateTime};

use crypto::hash::BlockHash;

use crate::p2p::encoding::block_header::{display_fitness, Level};

pub mod base;
pub mod p2p;
pub mod protocol;

#[derive(Debug, thiserror::Error)]
#[error("invalid or out-of-range datetime")]
pub struct TimestampOutOfRangeError;

impl From<ComponentRange> for TimestampOutOfRangeError {
    fn from(_error: ComponentRange) -> Self {
        Self
    }
}

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub fn ts_to_rfc3339(ts: i64) -> Result<String, TimestampOutOfRangeError> {
    Ok(OffsetDateTime::from_unix_timestamp(ts)?
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("invalid timestamp")))
}

/// This common struct holds info (hash, level, fitness) about block used as head,
/// e.g. for fast computations without need to access storage
/// (if you need here more attributes from block_header, consider refactor block_header with this struct as shell_header)
#[derive(Clone, Debug, Getters, Serialize, Deserialize)]
pub struct Head {
    // TODO: TE-369 - Arc refactor
    /// BlockHash of head.
    #[get = "pub"]
    block_hash: BlockHash,
    /// Level of the head.
    #[get = "pub"]
    level: Level,
    /// Fitness of block
    #[get = "pub"]
    fitness: Fitness,
}

impl Head {
    pub fn new(block_hash: BlockHash, level: Level, fitness: Fitness) -> Self {
        Self {
            block_hash,
            level,
            fitness,
        }
    }

    pub fn to_debug_info(&self) -> (String, Level, String) {
        (
            self.block_hash.to_base58_check(),
            self.level,
            display_fitness(&self.fitness),
        )
    }
}

impl From<Head> for BlockHash {
    fn from(h: Head) -> Self {
        h.block_hash
    }
}

impl From<Head> for Level {
    fn from(h: Head) -> Self {
        h.level
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Timestamp(i64);

impl NomReader for Timestamp {
    fn nom_read(input: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        nom::combinator::map(
            nom::number::complete::i64(nom::number::Endianness::Big),
            Timestamp,
        )(input)
    }
}

impl BinWriter for Timestamp {
    fn bin_write(&self, output: &mut Vec<u8>) -> tezos_encoding::enc::BinResult {
        tezos_encoding::enc::i64(&self.0, output)
    }
}

impl HasEncoding for Timestamp {
    fn encoding() -> Encoding {
        Encoding::Timestamp
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            match ts_to_rfc3339(self.0) {
                Ok(ts) => serializer.serialize_str(&ts),
                Err(err) => Err(serde::ser::Error::custom(format!(
                    "cannot convert timestamp to rfc3339: {err}"
                ))),
            }
        } else {
            serializer.serialize_newtype_struct("Timestamp", &self.0)
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct TimestampVisitor;

        impl<'de> serde::de::Visitor<'de> for TimestampVisitor {
            type Value = Timestamp;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("timestamp either as i64 or Rfc3339 is expected")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let parsed: OffsetDateTime = OffsetDateTime::parse(v, &Rfc3339)
                    .map_err(|e| E::custom(format!("error parsing Rfc3339 timestamp: {e}")))?;
                let timestamp = parsed.unix_timestamp();
                Ok(Timestamp(timestamp))
            }

            fn visit_newtype_struct<E>(self, e: E) -> Result<Self::Value, E::Error>
            where
                E: serde::Deserializer<'de>,
            {
                let field0 = match <i64 as serde::Deserialize>::deserialize(e) {
                    Ok(val) => val,
                    Err(err) => {
                        return Err(err);
                    }
                };
                Ok(Timestamp(field0))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let timestamp = match seq.next_element::<i64>() {
                    Ok(Some(val)) => val,
                    Ok(None) => return Err(serde::de::Error::custom("no timestamp".to_string())),
                    Err(err) => return Err(err),
                };
                Ok(Timestamp(timestamp))
            }
        }

        if deserializer.is_human_readable() {
            deserializer.deserialize_str(TimestampVisitor)
        } else {
            deserializer.deserialize_newtype_struct("Timestamp", TimestampVisitor)
        }
    }
}
