// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use tezos_api::client::TezosStorageInitInfo;
use tezos_api::environment::TezosEnvironment;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi::*;
use tezos_messages::p2p::encoding::prelude::*;

pub trait ProtocolApi {
    fn apply_block(chain_id: &ChainId, block_header_hash: &BlockHash, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError>;

    fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError>;

    fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, TezosStorageInitError>;
}
