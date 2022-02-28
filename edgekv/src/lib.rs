// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::errors::EdgeKVError;

pub mod datastore;
pub mod edgekv;
pub mod errors;
pub mod file_ops;
pub mod schema;
pub type Result<T> = std::result::Result<T, EdgeKVError>;

//#[cfg(test)]
//mod tests;
