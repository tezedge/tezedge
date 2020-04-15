// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::string::ToString;

use failure::bail;

use storage::num_from_slice;
use storage::skip_list::Bucket;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
use tezos_messages::protocol::UniversalValue;
// use tezos_messages::protocol::proto_005_2::delegate::{BalanceByCycle, Delegate, DelegateList};
use tezos_messages::p2p::binary_message::BinaryMessage;

use crate::helpers::ContextProtocolParam;

pub(crate) fn get_current_quorum(context_proto_params: ContextProtocolParam, _chain_id: &str, context: TezedgeContext) -> Result<Option<UniversalValue>, failure::Error> {
    
    // get quorum_min and quorum_max from the constants(protocol dependant)
    let dynamic = tezos_messages::protocol::proto_006::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;
    let quorum_min = dynamic.quorum_min();
    let quorum_max = dynamic.quorum_max();

    // calculate the quorum_diff -> (quorum_max - quorum_min)
    let quorum_diff = quorum_max - quorum_min;

    // get participation_ema from the context DB
    let context_index = ContextIndex::new(Some(context_proto_params.level.clone()), None);
    let participation_ema;
    if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &vec!["data/votes/participation_ema".to_string()])? {
        participation_ema = num_from_slice!(data, 0, i32);
    } else {
        bail!("Cannot get participation_ema from context");
    }
    // calcualte and return the current quorum -> quorum_min + ((participation_ema * quorum_diff) / 100_00)
    let current_quorum = quorum_min + ((participation_ema * quorum_diff) / 100_00);
    Ok(Some(UniversalValue::Number(current_quorum)))
}