// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use serde::{Serialize, Deserialize};
use tezos_messages::protocol::UniversalValue;

//use super::base_types::*;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct EndorsingRight {
    level: i32,
    delegate: String,
    slots: Vec<u16>,
    estimated_time: String
}

pub type EndorsingRights = Vec<EndorsingRight>;

impl EndorsingRight {
    pub fn transpile_context_bytes(value: &[u8]) -> Result<Self, failure::Error> {
        let data = tezos_messages::protocol::get_endorsing_data(value)?;
        let mut ret: Self = Default::default();
        if let Some(data) = data {
            if let Some(UniversalValue::NumberI32(val)) = data.get("level") {
                ret.level = val.clone();
            }
            if let Some(UniversalValue::String(val)) = data.get("delegate") {
                ret.delegate = val.clone();
            }
            if let Some(UniversalValue::ListU16(values)) = data.get("slots") {
                for x in values {
                    if let UniversalValue::NumberU16(val) = **x {
                        ret.slots.push(val);
                    }
                }
            }
            if let Some(UniversalValue::String(val)) = data.get("estimated_time") {
                ret.estimated_time = val.clone();
            }
        }
        Ok(ret)
    }
}

