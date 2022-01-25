// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use super::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutState;
use super::commit_result_get::StorageBlocksGenesisInitCommitResultGetState;
use super::commit_result_put::StorageBlocksGenesisInitCommitResultPutState;
use super::header_put::StorageBlocksGenesisInitHeaderPutState;

#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum StorageBlocksGenesisInitState {
    Idle,

    #[from]
    HeaderPut(StorageBlocksGenesisInitHeaderPutState),

    #[from]
    AdditionalDataPut(StorageBlocksGenesisInitAdditionalDataPutState),

    #[from]
    CommitResultGet(StorageBlocksGenesisInitCommitResultGetState),

    #[from]
    CommitResultPut(StorageBlocksGenesisInitCommitResultPutState),

    Success,
}
