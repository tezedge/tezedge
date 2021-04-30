// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Use intermediary type `IOErrorKind` to serialize/deserialize
//! io::ErrorKind, which can't be serialized/deserialized otherwise.
//!
//! Inside struct usage looks like this:
//! ```
//! use serde::{Serialize, Deserialize};
//! #[derive(Serialize, Deserialize)]
//! pub struct SomeProposal {
//!     #[serde(with = "io_error_kind::serde")]
//!     error: std::io::ErrorKind,
//! }
//! ```

use super::IOErrorKind;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::io::ErrorKind;

pub fn serialize<S>(value: &ErrorKind, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    IOErrorKind::from(*value).serialize(s)
}

pub fn deserialize<'de, D>(d: D) -> Result<ErrorKind, D::Error>
where
    D: Deserializer<'de>,
{
    IOErrorKind::deserialize(d).map(|x| x.into())
}
