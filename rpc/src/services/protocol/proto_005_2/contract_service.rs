// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::string::ToString;
use std::convert::TryInto;

use failure::bail;

use storage::context::{TezedgeContext, ContextApi};
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::binary_message::BinaryMessage;

use crate::helpers::ContextProtocolParam;
use crate::services::protocol::proto_005_2::helpers::{construct_indexed_contract_key};

pub(crate) fn get_contract_counter(context_proto_params: ContextProtocolParam, pkh: &str, context: TezedgeContext) -> Result<Option<String>, failure::Error> {

    // level of the block
    let level = context_proto_params.level;

    // get context_hash from level
    let ctx_hash = context.level_to_hash(level.try_into()?)?;
    
    let indexed_contract_key = construct_indexed_contract_key(pkh)?;

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","counter"]
    let contract_counter_key = vec![indexed_contract_key.clone(), "counter".to_string()];
    let contract_counter = if let Some(data) = context.get_key_from_history(&ctx_hash, &contract_counter_key)? {
        Some(tezos_messages::protocol::proto_005_2::contract::Counter::from_bytes(data)?)
    } else {
        None
    };

    if let Some(contract_counter) = contract_counter {
        Ok(Some(contract_counter.to_string()))
    } else {
        Ok(None)
    }
}

pub(crate) fn get_contract_manager_key(context_proto_params: ContextProtocolParam, pkh: &str, context: TezedgeContext) -> Result<Option<String>, failure::Error> {

    // level of the block
    let level = context_proto_params.level;

    // get context_hash from level
    let ctx_hash = context.level_to_hash(level.try_into()?)?;
    
    let indexed_contract_key = construct_indexed_contract_key(pkh)?;

    // ["data","contracts","index","91","6e","d7","72","4e","49","0000535110affdb82923710d1ec205f26ba8820a2259","manager"]
    let manager_key_key = vec![indexed_contract_key.clone(), "manager".to_string()];
    if let Some(data) = context.get_key_from_history(&ctx_hash, &manager_key_key)? {
        match SignaturePublicKey::from_tagged_bytes(data) {
            Ok(pk) => {
                Ok(Some(pk.to_string()))
            }
            Err(_) => bail!("Manager key not revealed yet")
        }
        
    } else {
        Ok(None)
    }
}
