// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use crypto::hash::{HashType, ProtocolHash};
use failure::Error;
use crate::p2p::binary_message::BinaryMessage;
use tezos_encoding::types::BigInt;
use std::collections::HashMap;

pub mod proto_000 {
    pub const PROTOCOL_HASH: &str = "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P";
}

pub mod proto_001;
pub mod proto_002;
pub mod proto_003;
pub mod proto_004;
pub mod proto_005;
pub mod proto_005_2;

pub enum UniversalValue {
    Number(i64),
    BigNumber(BigInt),
    List(Vec<Box<UniversalValue>>),
}

impl UniversalValue {
    fn num<T: Into<i64>>(val: T) -> Self {
        Self::Number(val.into())
    }

    fn big_num(val: BigInt) -> Self {
        Self::BigNumber(val)
    }

    fn num_list<'a, T: 'a + Into<i64> + Clone, I: IntoIterator<Item=&'a T>>(val: I) -> Self {
        let mut ret: Vec<Box<UniversalValue>> = Default::default();
        for x in val {
            ret.push(Box::new(Self::num(x.clone())))
        }
        Self::List(ret)
    }
}

pub fn get_constants(bytes: &[u8], protocol: ProtocolHash) -> Result<Option<HashMap<&'static str, UniversalValue>>, Error> {
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&protocol);
    match hash {
        proto_001::PROTOCOL_HASH => {
            use crate::protocol::proto_001::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_002::PROTOCOL_HASH => {
            use crate::protocol::proto_002::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_003::PROTOCOL_HASH => {
            use crate::protocol::proto_003::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_004::PROTOCOL_HASH => {
            use crate::protocol::proto_004::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_005::PROTOCOL_HASH => {
            use crate::protocol::proto_005::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        proto_005_2::PROTOCOL_HASH => {
            use crate::protocol::proto_005_2::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes.to_vec())?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        _ => Ok(None)
    }
}