// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
#![feature(concat_idents)]

//! This crate provides serialization and deserialization functionality for the data types used by the Tezos shell.

mod bit_utils;
pub mod types;

pub mod binary_async_reader;
pub mod binary_reader;
pub mod binary_writer;
pub mod de;
pub mod encoding;
pub mod error_context;
pub mod json_writer;
pub mod ser;
