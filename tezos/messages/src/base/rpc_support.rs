// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Simple support for rpc and json format conversions

use std::collections::HashMap;

use serde::ser::SerializeSeq;
use serde::{ser, Serialize};

use tezos_encoding::types::BigInt;

use crate::ts_to_rfc3339;

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
    pub fn num<T: Into<i32>>(val: T) -> Self {
        Self::Number(val.into())
    }

    pub fn string(val: String) -> Self {
        Self::String(val)
    }

    pub fn timestamp_rfc3339(val: i64) -> Self {
        Self::TimestampRfc3339(val)
    }

    pub fn i64(val: i64) -> Self {
        Self::NumberI64(val)
    }

    pub fn big_num(val: impl Into<BigInt>) -> Self {
        Self::BigNumber(val.into())
    }

    pub fn i64_list(val: Vec<i64>) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::i64(x))
        }
        Self::List(ret)
    }

    pub fn num_list<'a, T: 'a + Into<i32> + Clone, I: IntoIterator<Item = &'a T>>(val: I) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::num(x.clone()))
        }
        Self::List(ret)
    }

    pub fn big_num_list<I: IntoIterator<Item = T>, T: Into<BigInt>>(val: I) -> Self {
        let mut ret: Vec<UniversalValue> = Default::default();
        for x in val {
            ret.push(Self::big_num(x.into()))
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
            UniversalValue::BigNumber(num) => serializer.serialize_str(&format!("{}", num.0)),
            UniversalValue::Number(num) => serializer.serialize_i32(*num),
            UniversalValue::NumberI64(num) => serializer.serialize_str(num.to_string().as_str()),
            UniversalValue::String(val) => serializer.serialize_str(val.as_str()),
            UniversalValue::TimestampRfc3339(val) => {
                let timestamp = ts_to_rfc3339(*val).map_err(ser::Error::custom)?;
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
