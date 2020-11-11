// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use failure::Error;
use serde::{ser, Serialize};
use serde::ser::SerializeSeq;

use crypto::hash::{HashType, ProtocolHash};
use tezos_encoding::types::BigInt;

use crate::p2p::binary_message::BinaryMessage;
use crate::ts_to_rfc3339;

pub mod proto_001;
pub mod proto_002;
pub mod proto_003;
pub mod proto_004;
pub mod proto_005;
pub mod proto_005_2;
pub mod proto_006;
pub mod proto_007;

#[derive(Debug, Clone)]
pub enum UniversalValue {
    Number(i32),
    /// Ocaml RPC formats i64 as string
    NumberI64(i64),
    BigNumber(BigInt),
    List(Vec<UniversalValue>),
    String(String),
    TimestampRfc3339(i64),
}

impl UniversalValue {
    fn num<T: Into<i32>>(val: T) -> Self {
        Self::Number(val.into())
    }

    fn string(val: String) -> Self {
        Self::String(val)
    }

    fn timestamp_rfc3339(val: i64) -> Self {
        Self::TimestampRfc3339(val)
    }

    fn i64(val: i64) -> Self {
        Self::NumberI64(val)
    }

    fn big_num(val: BigInt) -> Self {
        Self::BigNumber(val)
    }

    fn i64_list(val: Vec<i64>) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::i64(x))
        }
        Self::List(ret)
    }

    fn num_list<'a, T: 'a + Into<i32> + Clone, I: IntoIterator<Item=&'a T>>(val: I) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::num(x.clone()))
        }
        Self::List(ret)
    }

    fn big_num_list<I: IntoIterator<Item=BigInt>>(val: I) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::big_num(x.clone()))
        }
        Self::List(ret)
    }
}

impl Serialize for UniversalValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: ser::Serializer,
    {
        match self {
            UniversalValue::BigNumber(num) => {
                serializer.serialize_str(&format!("{}", num.0))
            }
            UniversalValue::Number(num) => {
                serializer.serialize_i32(*num)
            }
            UniversalValue::NumberI64(num) => {
                serializer.serialize_str(num.to_string().as_str())
            }
            UniversalValue::String(val) => {
                serializer.serialize_str(val.as_str())
            }
            UniversalValue::TimestampRfc3339(val) => {
                let timestamp = ts_to_rfc3339(*val);
                serializer.serialize_str(timestamp.as_str())
            }
            UniversalValue::List(values) => {
                let mut seq = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    seq.serialize_element(value)?;
                }
                seq.end()
            }
        }
    }
}

pub type RpcJsonMap = HashMap<&'static str, UniversalValue>;

/// A trait for converting a protocol data for RPC json purposes.
pub trait ToRpcJsonMap {
    /// Converts a value of `self` to a HashMap, which can be serialized as json for rpc
    fn as_map(&self) -> RpcJsonMap;
}

pub fn get_constants_for_rpc(bytes: &[u8], protocol: ProtocolHash) -> Result<Option<RpcJsonMap>, Error> {
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&protocol);
    match hash {
        proto_001::PROTOCOL_HASH => {
            use crate::protocol::proto_001::constants::{ParametricConstants, FIXED};
            let context_param = ParametricConstants::from_bytes(bytes)?;
            
            let param = ParametricConstants::create_with_default_and_merge(context_param);

            let mut param_map = param.as_map();
            param_map.extend(FIXED.clone().as_map());
            Ok(Some(param_map))
        }
        proto_002::PROTOCOL_HASH => {
            use crate::protocol::proto_002::constants::{ParametricConstants, FIXED};
            let context_param = ParametricConstants::from_bytes(bytes)?;

            let param = ParametricConstants::create_with_default_and_merge(context_param);

            let mut param_map = param.as_map();
            param_map.extend(FIXED.clone().as_map());
            Ok(Some(param_map))
        }
        proto_003::PROTOCOL_HASH => {
            use crate::protocol::proto_003::constants::{ParametricConstants, FIXED};
            let context_param = ParametricConstants::from_bytes(bytes)?;
            
            let param = ParametricConstants::create_with_default_and_merge(context_param);

            let mut param_map = param.as_map();
            param_map.extend(FIXED.clone().as_map());
            Ok(Some(param_map))
        }
        proto_004::PROTOCOL_HASH => {
            use crate::protocol::proto_004::constants::{ParametricConstants, FIXED};
            let context_param = ParametricConstants::from_bytes(bytes)?;
            
            let param = ParametricConstants::create_with_default_and_merge(context_param);

            let mut param_map = param.as_map();
            param_map.extend(FIXED.clone().as_map());
            Ok(Some(param_map))
        }
        proto_005::PROTOCOL_HASH => {
            use crate::protocol::proto_005::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes)?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_005_2::PROTOCOL_HASH => {
            use crate::protocol::proto_005_2::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes)?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_006::PROTOCOL_HASH => {
            use crate::protocol::proto_006::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes)?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_007::PROTOCOL_HASH => {
            use crate::protocol::proto_007::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes)?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        _ => panic!("Missing constants encoding for protocol: {}, protocol is not yet supported!", hash)
    }
}