// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ChainId;
use tezos_api::client::TezosStorageInitInfo;
use tezos_api::environment::TezosEnvironment;
use tezos_api::ffi::*;
use tezos_api::identity::Identity;
use tezos_messages::p2p::encoding::prelude::*;

/// Provides trait that must be implemented by a protocol runner.
pub trait ProtocolApi {

    /// Apply block
    fn apply_block(chain_id: &ChainId, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError>;

    /// Change tezos runtime configuration
    fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError>;

    /// Command tezos ocaml code to initialize storage.
    fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment, enable_testchain: bool) -> Result<TezosStorageInitInfo, TezosStorageInitError>;

    /// Command tezos ocaml code to generate a new identity.
    fn generate_identity(expected_pow: f64) -> Result<Identity, TezosGenerateIdentityError>;
}
