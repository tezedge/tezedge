// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides serialization and deserialization functionality for the data types used by the Tezos shell.


pub mod types;
mod bit_utils;

pub mod encoding;
pub mod de;
pub mod ser;
pub mod binary_reader;
pub mod binary_writer;
pub mod json_writer;