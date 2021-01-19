// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;

use crypto::hash::{HashType, ProtocolHash};

use crate::base::rpc_support::{RpcJsonMap, ToRpcJsonMap};
use crate::p2p::binary_message::BinaryMessage;

pub mod proto_001;
pub mod proto_002;
pub mod proto_003;
pub mod proto_004;
pub mod proto_005;
pub mod proto_005_2;
pub mod proto_006;
pub mod proto_007;
pub mod proto_008;

pub fn get_constants_for_rpc(
    bytes: &[u8],
    protocol: ProtocolHash,
) -> Result<Option<RpcJsonMap>, Error> {
    let hash: &str = &HashType::ProtocolHash.hash_to_b58check(&protocol);
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
        proto_008::PROTOCOL_HASH => {
            use crate::protocol::proto_008::constants::{ParametricConstants, FIXED};
            let mut param = ParametricConstants::from_bytes(bytes)?.as_map();
            param.extend(FIXED.clone().as_map());
            Ok(Some(param))
        }
        _ => panic!(
            "Missing constants encoding for protocol: {}, protocol is not yet supported!",
            hash
        ),
    }
}
