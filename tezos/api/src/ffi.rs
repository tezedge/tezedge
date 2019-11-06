// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_new::new;
use serde::{Deserialize, Serialize};

pub type RustBytes = Vec<u8>;

#[derive(Debug)]
pub struct GenesisChain {
    pub time: String,
    pub block: String,
    pub protocol: String,
}

#[derive(Debug)]
pub struct ProtocolOverrides {
    pub forced_protocol_upgrades: Vec<(i32, String)>,
    pub voted_protocol_overrides: Vec<(String, String)>,
}

#[derive(Debug)]
pub struct OcamlStorageInitInfo {
    pub chain_id: RustBytes,
    pub test_chain: Option<TestChain>,
    pub genesis_block_header_hash: RustBytes,
    pub genesis_block_header: RustBytes,
    pub current_block_header_hash: RustBytes,
    pub supported_protocol_hashes: Vec<RustBytes>,
}

#[derive(Debug, new, Serialize, Deserialize)]
pub struct TestChain {
    pub chain_id: RustBytes,
    pub protocol_hash: RustBytes,
    pub expiration_date: String,
}

/// Holds configuration for ocaml runtime - e.g. arguments which are passed to ocaml and can be change in runtime
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosRuntimeConfiguration {
    pub log_enabled: bool
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ApplyBlockResult {
    pub validation_result_message: String,
    pub context_hash: RustBytes,
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_json: String,
}