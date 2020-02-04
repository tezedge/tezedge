// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides serialization and deserialization functionality for the data types used by the Tezos shell.


/// Defines types of the intermediate data format.
pub mod types;
mod bit_utils;
/// Schema used by for serialization and deserialization.
pub mod encoding;
/// Serde Deserializer
pub mod de;
/// Serde Serializer
pub mod ser;
/// Tezos binary data reader.
pub mod binary_reader;
/// Tezos binary data writer.
pub mod binary_writer;
/// Tezos json data writer.
pub mod json_writer;