// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "fuzzing")]
#![cfg_attr(test, feature(no_coverage))]

use fuzzcheck::{DefaultMutator, SerdeSerializer};
use redux_rs::{ActionId, ActionWithMeta, EnablingCondition, SafetyCondition};
use serde::{Deserialize, Serialize};
use shell_automaton::action::{Action, InitAction};
use shell_automaton::block_applier;
use shell_automaton::current_head::current_head_actions;
use shell_automaton::fuzzing::state_singleton::FUZZER_STATE;
use shell_automaton::mempool::mempool_actions;
use shell_automaton::peers::init::PeersInitAction;
use shell_automaton::prechecker::prechecker_actions;
use shell_automaton::protocol::ProtocolAction;
use shell_automaton::protocol_runner;
use shell_automaton::rights::rights_actions;
use shell_automaton::shutdown::ShutdownInitAction;
use shell_automaton::shutdown::ShutdownPendingAction;
use shell_automaton::shutdown::ShutdownSuccessAction;
use shell_automaton::stats::current_head::stats_current_head_actions;
//use shell_automaton::storage::request::StorageRequestSuccessAction;
use shell_automaton::MioWaitForEventsAction;

use shell_automaton::MioTimeoutEvent;
use shell_automaton::State;
use shell_automaton::{
    event::{P2pPeerEvent, P2pServerEvent, WakeupEvent},
    paused_loops::{
        PausedLoopsAddAction, PausedLoopsResumeAllAction, PausedLoopsResumeNextInitAction,
        PausedLoopsResumeNextSuccessAction,
    },
    peer::{
        binary_message::{
            read::{
                PeerBinaryMessageReadChunkReadyAction, PeerBinaryMessageReadErrorAction,
                PeerBinaryMessageReadInitAction, PeerBinaryMessageReadReadyAction,
                PeerBinaryMessageReadSizeReadyAction,
            },
            write::{
                PeerBinaryMessageWriteErrorAction, PeerBinaryMessageWriteNextChunkAction,
                PeerBinaryMessageWriteReadyAction, PeerBinaryMessageWriteSetContentAction,
            },
        },
        chunk::{
            read::{
                PeerChunkReadDecryptAction, PeerChunkReadErrorAction, PeerChunkReadInitAction,
                PeerChunkReadPartAction, PeerChunkReadReadyAction,
            },
            write::{
                PeerChunkWriteCreateChunkAction, PeerChunkWriteEncryptContentAction,
                PeerChunkWriteErrorAction, PeerChunkWritePartAction, PeerChunkWriteReadyAction,
                PeerChunkWriteSetContentAction,
            },
        },
        connection::{
            closed::PeerConnectionClosedAction,
            incoming::{
                accept::{
                    PeerConnectionIncomingAcceptAction, PeerConnectionIncomingAcceptErrorAction,
                    PeerConnectionIncomingAcceptSuccessAction,
                    PeerConnectionIncomingRejectedAction,
                },
                PeerConnectionIncomingErrorAction, PeerConnectionIncomingSuccessAction,
            },
            outgoing::{
                PeerConnectionOutgoingErrorAction, PeerConnectionOutgoingInitAction,
                PeerConnectionOutgoingPendingAction, PeerConnectionOutgoingRandomInitAction,
                PeerConnectionOutgoingSuccessAction,
            },
        },
        disconnection::{PeerDisconnectAction, PeerDisconnectedAction},
        handshaking::{
            PeerHandshakingAckMessageDecodeAction, PeerHandshakingAckMessageEncodeAction,
            PeerHandshakingAckMessageInitAction, PeerHandshakingAckMessageReadAction,
            PeerHandshakingAckMessageWriteAction, PeerHandshakingConnectionMessageDecodeAction,
            PeerHandshakingConnectionMessageEncodeAction,
            PeerHandshakingConnectionMessageInitAction, PeerHandshakingConnectionMessageReadAction,
            PeerHandshakingConnectionMessageWriteAction, PeerHandshakingEncryptionInitAction,
            PeerHandshakingErrorAction, PeerHandshakingFinishAction, PeerHandshakingInitAction,
            PeerHandshakingMetadataMessageDecodeAction, PeerHandshakingMetadataMessageEncodeAction,
            PeerHandshakingMetadataMessageInitAction, PeerHandshakingMetadataMessageReadAction,
            PeerHandshakingMetadataMessageWriteAction,
        },
        message::{
            read::{
                PeerMessageReadErrorAction, PeerMessageReadInitAction, PeerMessageReadSuccessAction,
            },
            write::{
                PeerMessageWriteErrorAction, PeerMessageWriteInitAction,
                PeerMessageWriteNextAction, PeerMessageWriteSuccessAction,
            },
        },
        PeerTryReadLoopFinishAction, PeerTryReadLoopStartAction, PeerTryWriteLoopFinishAction,
        PeerTryWriteLoopStartAction,
    },
    peers::{
        add::{multi::PeersAddMultiAction, PeersAddIncomingPeerAction},
        check::timeouts::{
            PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction,
            PeersCheckTimeoutsSuccessAction,
        },
        dns_lookup::{
            PeersDnsLookupCleanupAction, PeersDnsLookupErrorAction, PeersDnsLookupInitAction,
            PeersDnsLookupSuccessAction,
        },
        graylist::{
            PeersGraylistAddressAction, PeersGraylistIpAddAction, PeersGraylistIpAddedAction,
            PeersGraylistIpRemoveAction, PeersGraylistIpRemovedAction,
        },
        remove::PeersRemoveAction,
    },
    storage,
};

/*


pub enum Action {
}


*/

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum AllActionsTest {
    TestControl(ControlActionTest),
    TestPeersDnsAction(PeersDnsLookupActionTest),
    TestPeerActions(PeerActionTest),
    TestStorage(StorageActionTest),
    TestMempool(MempoolActionTest),
    TestPrechecker(PrecheckerActionTest),
    TestProtocolRunner(ProtocolRunnerActionTest),
    TestBlockApplier(BlockApplierActionTest),
    TestRights(RightsActionTest),
    TestCurrentHead(CurrentHeadActionTest),
    TestStats(StatsActionTest),
}

impl AllActionsTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestControl(a) => a.to_action(),
            Self::TestPeersDnsAction(a) => a.to_action(),
            Self::TestPeerActions(a) => a.to_action(),
            Self::TestStorage(a) => a.to_action(),
            Self::TestMempool(a) => a.to_action(),
            Self::TestPrechecker(a) => a.to_action(),
            Self::TestProtocolRunner(a) => a.to_action(),
            Self::TestBlockApplier(a) => a.to_action(),
            Self::TestRights(a) => a.to_action(),
            Self::TestCurrentHead(a) => a.to_action(),
            Self::TestStats(a) => a.to_action(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum StatsActionTest {
    TestStatsCurrentHeadReceivedAction(stats_current_head_actions::StatsCurrentHeadReceivedAction),
    TestStatsCurrentHeadPrecheckSuccessAction(
        stats_current_head_actions::StatsCurrentHeadPrecheckSuccessAction,
    ),
    TestStatsCurrentHeadPrepareSendAction(
        stats_current_head_actions::StatsCurrentHeadPrepareSendAction,
    ),
    TestStatsCurrentHeadSentAction(stats_current_head_actions::StatsCurrentHeadSentAction),
    TestStatsCurrentHeadSentErrorAction(
        stats_current_head_actions::StatsCurrentHeadSentErrorAction,
    ),
    //TestStatsCurrentHeadRpcGetAction(stats_current_head_actions::StatsCurrentHeadRpcGetAction),
    TestStatsCurrentHeadPruneAction(stats_current_head_actions::StatsCurrentHeadPruneAction),
    TestStatsCurrentHeadRpcGetPeersAction(
        stats_current_head_actions::StatsCurrentHeadRpcGetPeersAction,
    ),
    TestStatsCurrentHeadRpcGetApplicationAction(
        stats_current_head_actions::StatsCurrentHeadRpcGetApplicationAction,
    ),
}

impl StatsActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestStatsCurrentHeadReceivedAction(a) => a.into(),
            Self::TestStatsCurrentHeadPrecheckSuccessAction(a) => a.into(),
            Self::TestStatsCurrentHeadPrepareSendAction(a) => a.into(),
            Self::TestStatsCurrentHeadSentAction(a) => a.into(),
            Self::TestStatsCurrentHeadSentErrorAction(a) => a.into(),
            //Self::TestStatsCurrentHeadRpcGetAction(a) => a.into(),
            Self::TestStatsCurrentHeadPruneAction(a) => a.into(),
            Self::TestStatsCurrentHeadRpcGetPeersAction(a) => a.into(),
            Self::TestStatsCurrentHeadRpcGetApplicationAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum CurrentHeadActionTest {
    TestCurrentHeadReceivedAction(current_head_actions::CurrentHeadReceivedAction),
    TestCurrentHeadPrecheckAction(current_head_actions::CurrentHeadPrecheckAction),
    TestCurrentHeadPrecheckSuccessAction(current_head_actions::CurrentHeadPrecheckSuccessAction),
    TestCurrentHeadPrecheckRejectedAction(current_head_actions::CurrentHeadPrecheckRejectedAction),
    TestCurrentHeadErrorAction(current_head_actions::CurrentHeadErrorAction),
    TestCurrentHeadApplyAction(current_head_actions::CurrentHeadApplyAction),
    TestCurrentHeadPrecacheBakingRightsAction(
        current_head_actions::CurrentHeadPrecacheBakingRightsAction,
    ),
}

impl CurrentHeadActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestCurrentHeadReceivedAction(a) => a.into(),
            Self::TestCurrentHeadPrecheckAction(a) => a.into(),
            Self::TestCurrentHeadPrecheckSuccessAction(a) => a.into(),
            Self::TestCurrentHeadPrecheckRejectedAction(a) => a.into(),
            Self::TestCurrentHeadErrorAction(a) => a.into(),
            Self::TestCurrentHeadApplyAction(a) => a.into(),
            Self::TestCurrentHeadPrecacheBakingRightsAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum RightsActionTest {
    TestRightsGetAction(rights_actions::RightsGetAction),
    TestRightsRpcGetAction(rights_actions::RightsRpcGetAction),
    TestRightsRpcEndorsingReadyAction(rights_actions::RightsRpcEndorsingReadyAction),
    TestRightsRpcBakingReadyAction(rights_actions::RightsRpcBakingReadyAction),
    TestRightsRpcErrorAction(rights_actions::RightsRpcErrorAction),
    TestRightsRpcPruneAction(rights_actions::RightsRpcPruneAction),
    TestRightsInitAction(rights_actions::RightsInitAction),
    TestRightsGetBlockHeaderAction(rights_actions::RightsGetBlockHeaderAction),
    TestRightsBlockHeaderReadyAction(rights_actions::RightsBlockHeaderReadyAction),
    TestRightsGetProtocolHashAction(rights_actions::RightsGetProtocolHashAction),
    TestRightsProtocolHashReadyAction(rights_actions::RightsProtocolHashReadyAction),
    TestRightsGetProtocolConstantsAction(rights_actions::RightsGetProtocolConstantsAction),
    TestRightsProtocolConstantsReadyAction(rights_actions::RightsProtocolConstantsReadyAction),
    TestRightsGetCycleErasAction(rights_actions::RightsGetCycleErasAction),
    TestRightsCycleErasReadyAction(rights_actions::RightsCycleErasReadyAction),
    TestRightsGetCycleAction(rights_actions::RightsGetCycleAction),
    TestRightsCycleReadyAction(rights_actions::RightsCycleReadyAction),
    TestRightsGetCycleDataAction(rights_actions::RightsGetCycleDataAction),
    TestRightsCycleDataReadyAction(rights_actions::RightsCycleDataReadyAction),
    TestRightsCalculateAction(rights_actions::RightsCalculateAction),
    TestRightsEndorsingReadyAction(rights_actions::RightsEndorsingReadyAction),
    TestRightsBakingReadyAction(rights_actions::RightsBakingReadyAction),
    TestRightsErrorAction(rights_actions::RightsErrorAction),
}

impl RightsActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestRightsGetAction(a) => a.into(),
            Self::TestRightsRpcGetAction(a) => a.into(),
            Self::TestRightsRpcEndorsingReadyAction(a) => a.into(),
            Self::TestRightsRpcBakingReadyAction(a) => a.into(),
            Self::TestRightsRpcErrorAction(a) => a.into(),
            Self::TestRightsRpcPruneAction(a) => a.into(),
            Self::TestRightsInitAction(a) => a.into(),
            Self::TestRightsGetBlockHeaderAction(a) => a.into(),
            Self::TestRightsBlockHeaderReadyAction(a) => a.into(),
            Self::TestRightsGetProtocolHashAction(a) => a.into(),
            Self::TestRightsProtocolHashReadyAction(a) => a.into(),
            Self::TestRightsGetProtocolConstantsAction(a) => a.into(),
            Self::TestRightsProtocolConstantsReadyAction(a) => a.into(),
            Self::TestRightsGetCycleErasAction(a) => a.into(),
            Self::TestRightsCycleErasReadyAction(a) => a.into(),
            Self::TestRightsGetCycleAction(a) => a.into(),
            Self::TestRightsCycleReadyAction(a) => a.into(),
            Self::TestRightsGetCycleDataAction(a) => a.into(),
            Self::TestRightsCycleDataReadyAction(a) => a.into(),
            Self::TestRightsCalculateAction(a) => a.into(),
            Self::TestRightsEndorsingReadyAction(a) => a.into(),
            Self::TestRightsBakingReadyAction(a) => a.into(),
            Self::TestRightsErrorAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum BlockApplierActionTest {
    TestBlockApplierEnqueueBlockAction(block_applier::BlockApplierEnqueueBlockAction),
    TestBlockApplierApplyInitAction(block_applier::BlockApplierApplyInitAction),
    TestBlockApplierApplyPrepareDataPendingAction(
        block_applier::BlockApplierApplyPrepareDataPendingAction,
    ),
    TestBlockApplierApplyPrepareDataSuccessAction(
        block_applier::BlockApplierApplyPrepareDataSuccessAction,
    ),
    TestBlockApplierApplyProtocolRunnerApplyInitAction(
        block_applier::BlockApplierApplyProtocolRunnerApplyInitAction,
    ),
    TestBlockApplierApplyProtocolRunnerApplyPendingAction(
        block_applier::BlockApplierApplyProtocolRunnerApplyPendingAction,
    ),
    TestBlockApplierApplyProtocolRunnerApplyRetryAction(
        block_applier::BlockApplierApplyProtocolRunnerApplyRetryAction,
    ),
    TestBlockApplierApplyProtocolRunnerApplySuccessAction(
        block_applier::BlockApplierApplyProtocolRunnerApplySuccessAction,
    ),
    TestBlockApplierApplyStoreApplyResultPendingAction(
        block_applier::BlockApplierApplyStoreApplyResultPendingAction,
    ),
    TestBlockApplierApplyStoreApplyResultSuccessAction(
        block_applier::BlockApplierApplyStoreApplyResultSuccessAction,
    ),
    TestBlockApplierApplyErrorAction(block_applier::BlockApplierApplyErrorAction),
    TestBlockApplierApplySuccessAction(block_applier::BlockApplierApplySuccessAction),
}

impl BlockApplierActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestBlockApplierEnqueueBlockAction(a) => a.into(),
            Self::TestBlockApplierApplyInitAction(a) => a.into(),
            Self::TestBlockApplierApplyPrepareDataPendingAction(a) => a.into(),
            Self::TestBlockApplierApplyPrepareDataSuccessAction(a) => a.into(),
            Self::TestBlockApplierApplyProtocolRunnerApplyInitAction(a) => a.into(),
            Self::TestBlockApplierApplyProtocolRunnerApplyPendingAction(a) => a.into(),
            Self::TestBlockApplierApplyProtocolRunnerApplyRetryAction(a) => a.into(),
            Self::TestBlockApplierApplyProtocolRunnerApplySuccessAction(a) => a.into(),
            Self::TestBlockApplierApplyStoreApplyResultPendingAction(a) => a.into(),
            Self::TestBlockApplierApplyStoreApplyResultSuccessAction(a) => a.into(),
            Self::TestBlockApplierApplyErrorAction(a) => a.into(),
            Self::TestBlockApplierApplySuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum ProtocolRunnerActionTest {
    TestProtocolRunnerStartAction(protocol_runner::ProtocolRunnerStartAction),
    TestProtocolRunnerSpawnServerInitAction(
        protocol_runner::spawn_server::ProtocolRunnerSpawnServerInitAction,
    ),
    TestProtocolRunnerSpawnServerPendingAction(
        protocol_runner::spawn_server::ProtocolRunnerSpawnServerPendingAction,
    ),
    TestProtocolRunnerSpawnServerErrorAction(
        protocol_runner::spawn_server::ProtocolRunnerSpawnServerErrorAction,
    ),
    TestProtocolRunnerSpawnServerSuccessAction(
        protocol_runner::spawn_server::ProtocolRunnerSpawnServerSuccessAction,
    ),
    TestProtocolRunnerInitAction(protocol_runner::init::ProtocolRunnerInitAction),
    TestProtocolRunnerInitRuntimeAction(
        protocol_runner::init::runtime::ProtocolRunnerInitRuntimeAction,
    ),
    TestProtocolRunnerInitRuntimePendingAction(
        protocol_runner::init::runtime::ProtocolRunnerInitRuntimePendingAction,
    ),
    TestProtocolRunnerInitRuntimeErrorAction(
        protocol_runner::init::runtime::ProtocolRunnerInitRuntimeErrorAction,
    ),
    TestProtocolRunnerInitRuntimeSuccessAction(
        protocol_runner::init::runtime::ProtocolRunnerInitRuntimeSuccessAction,
    ),
    TestProtocolRunnerInitCheckGenesisAppliedAction(
        protocol_runner::init::ProtocolRunnerInitCheckGenesisAppliedAction,
    ),
    TestProtocolRunnerInitCheckGenesisAppliedSuccessAction(
        protocol_runner::init::ProtocolRunnerInitCheckGenesisAppliedSuccessAction,
    ),
    TestProtocolRunnerInitContextAction(
        protocol_runner::init::context::ProtocolRunnerInitContextAction,
    ),
    TestProtocolRunnerInitContextPendingAction(
        protocol_runner::init::context::ProtocolRunnerInitContextPendingAction,
    ),
    TestProtocolRunnerInitContextErrorAction(
        protocol_runner::init::context::ProtocolRunnerInitContextErrorAction,
    ),
    TestProtocolRunnerInitContextSuccessAction(
        protocol_runner::init::context::ProtocolRunnerInitContextSuccessAction,
    ),
    TestProtocolRunnerInitContextIpcServerAction(
        protocol_runner::init::context_ipc_server::ProtocolRunnerInitContextIpcServerAction,
    ),
    TestProtocolRunnerInitContextIpcServerPendingAction(
        protocol_runner::init::context_ipc_server::ProtocolRunnerInitContextIpcServerPendingAction,
    ),
    TestProtocolRunnerInitContextIpcServerErrorAction(
        protocol_runner::init::context_ipc_server::ProtocolRunnerInitContextIpcServerErrorAction,
    ),
    TestProtocolRunnerInitContextIpcServerSuccessAction(
        protocol_runner::init::context_ipc_server::ProtocolRunnerInitContextIpcServerSuccessAction,
    ),
    TestProtocolRunnerInitSuccessAction(protocol_runner::init::ProtocolRunnerInitSuccessAction),
    TestProtocolRunnerReadyAction(protocol_runner::ProtocolRunnerReadyAction),
    TestProtocolRunnerNotifyStatusAction(protocol_runner::ProtocolRunnerNotifyStatusAction),
    TestProtocolRunnerResponseAction(protocol_runner::ProtocolRunnerResponseAction),
    TestProtocolRunnerResponseUnexpectedAction(
        protocol_runner::ProtocolRunnerResponseUnexpectedAction,
    ),
    TestProtocolRunnerShutdownInitAction(protocol_runner::ProtocolRunnerShutdownInitAction),
    TestProtocolRunnerShutdownPendingAction(protocol_runner::ProtocolRunnerShutdownPendingAction),
    TestProtocolRunnerShutdownSuccessAction(protocol_runner::ProtocolRunnerShutdownSuccessAction),
}

impl ProtocolRunnerActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestProtocolRunnerStartAction(a) => a.into(),
            Self::TestProtocolRunnerSpawnServerInitAction(a) => a.into(),
            Self::TestProtocolRunnerSpawnServerPendingAction(a) => a.into(),
            Self::TestProtocolRunnerSpawnServerErrorAction(a) => a.into(),
            Self::TestProtocolRunnerSpawnServerSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerInitAction(a) => a.into(),
            Self::TestProtocolRunnerInitRuntimeAction(a) => a.into(),
            Self::TestProtocolRunnerInitRuntimePendingAction(a) => a.into(),
            Self::TestProtocolRunnerInitRuntimeErrorAction(a) => a.into(),
            Self::TestProtocolRunnerInitRuntimeSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerInitCheckGenesisAppliedAction(a) => a.into(),
            Self::TestProtocolRunnerInitCheckGenesisAppliedSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextPendingAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextErrorAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextIpcServerAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextIpcServerPendingAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextIpcServerErrorAction(a) => a.into(),
            Self::TestProtocolRunnerInitContextIpcServerSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerInitSuccessAction(a) => a.into(),
            Self::TestProtocolRunnerReadyAction(a) => a.into(),
            Self::TestProtocolRunnerNotifyStatusAction(a) => a.into(),
            Self::TestProtocolRunnerResponseAction(a) => a.into(),
            Self::TestProtocolRunnerResponseUnexpectedAction(a) => a.into(),
            Self::TestProtocolRunnerShutdownInitAction(a) => a.into(),
            Self::TestProtocolRunnerShutdownPendingAction(a) => a.into(),
            Self::TestProtocolRunnerShutdownSuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum StorageActionTest {
    //TestStorageRequestCreateAction(storage::request::StorageRequestCreateAction),
    TestStorageRequestInitAction(storage::request::StorageRequestInitAction),
    TestStorageRequestPendingAction(storage::request::StorageRequestPendingAction),
    TestStorageResponseReceivedAction(storage::request::StorageResponseReceivedAction),
    TestStorageRequestErrorAction(storage::request::StorageRequestErrorAction),
    TestStorageRequestSuccessAction(storage::request::StorageRequestSuccessAction),
    TestStorageRequestFinishAction(storage::request::StorageRequestFinishAction),
    TestStorageStateSnapshotCreateInitAction(
        storage::state_snapshot::create::StorageStateSnapshotCreateInitAction,
    ),
    TestStorageStateSnapshotCreatePendingAction(
        storage::state_snapshot::create::StorageStateSnapshotCreatePendingAction,
    ),
    TestStorageStateSnapshotCreateErrorAction(
        storage::state_snapshot::create::StorageStateSnapshotCreateErrorAction,
    ),
    TestStorageStateSnapshotCreateSuccessAction(
        storage::state_snapshot::create::StorageStateSnapshotCreateSuccessAction,
    ),
    /*
    TestStorageBlockHeaderGetAction(storage::kv_block_header::StorageBlockHeaderGetAction),
    TestStorageBlockHeaderOkAction(storage::kv_block_header::StorageBlockHeaderOkAction),
    TestStorageBlockHeaderErrorAction(storage::kv_block_header::StorageBlockHeaderErrorAction),
    TestStorageBlockMetaGetAction(storage::kv_block_meta::StorageBlockMetaGetAction),
    TestStorageBlockMetaOkAction(storage::kv_block_meta::StorageBlockMetaOkAction),
    TestStorageBlockMetaErrorAction(storage::kv_block_meta::StorageBlockMetaErrorAction),
    TestStorageOperationsGetAction(storage::kv_operations::StorageOperationsGetAction),
    TestStorageOperationsOkAction(storage::kv_operations::StorageOperationsOkAction),
    TestStorageOperationsErrorAction(storage::kv_operations::StorageOperationsErrorAction),
    TestStorageBlockAdditionalDataGetAction(
        storage::kv_block_additional_data::StorageBlockAdditionalDataGetAction,
    ),
    TestStorageBlockAdditionalDataOkAction(
        storage::kv_block_additional_data::StorageBlockAdditionalDataOkAction,
    ),
    TestStorageBlockAdditionalDataErrorAction(
        storage::kv_block_additional_data::StorageBlockAdditionalDataErrorAction,
    ),
    TestStorageConstantsGetAction(storage::kv_constants::StorageConstantsGetAction),
    TestStorageConstantsOkAction(storage::kv_constants::StorageConstantsOkAction),
    TestStorageConstantsErrorAction(storage::kv_constants::StorageConstantsErrorAction),
    TestStorageCycleMetaGetAction(storage::kv_cycle_meta::StorageCycleMetaGetAction),
    TestStorageCycleMetaOkAction(storage::kv_cycle_meta::StorageCycleMetaOkAction),
    TestStorageCycleMetaErrorAction(storage::kv_cycle_meta::StorageCycleMetaErrorAction),
    TestStorageCycleErasGetAction(storage::kv_cycle_eras::StorageCycleErasGetAction),
    TestStorageCycleErasOkAction(storage::kv_cycle_eras::StorageCycleErasOkAction),
    TestStorageCycleErasErrorAction(storage::kv_cycle_eras::StorageCycleErasErrorAction),
    */
    TestStorageBlocksGenesisCheckAppliedInitAction(
        storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedInitAction,
    ),
    TestStorageBlocksGenesisCheckAppliedGetMetaPendingAction(
        storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedGetMetaPendingAction,
    ),
    TestStorageBlocksGenesisCheckAppliedGetMetaErrorAction(
        storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedGetMetaErrorAction,
    ),
    TestStorageBlocksGenesisCheckAppliedGetMetaSuccessAction(
        storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedGetMetaSuccessAction,
    ),
    TestStorageBlocksGenesisCheckAppliedSuccessAction(
        storage::blocks::genesis::check_applied::StorageBlocksGenesisCheckAppliedSuccessAction,
    ),
    TestStorageBlocksGenesisInitAction(storage::blocks::genesis::init::StorageBlocksGenesisInitAction),
    TestStorageBlocksGenesisInitHeaderPutInitAction(
        storage::blocks::genesis::init::header_put::StorageBlocksGenesisInitHeaderPutInitAction,
    ),
    TestStorageBlocksGenesisInitHeaderPutPendingAction(
        storage::blocks::genesis::init::header_put::StorageBlocksGenesisInitHeaderPutPendingAction,
    ),
    TestStorageBlocksGenesisInitHeaderPutErrorAction(
        storage::blocks::genesis::init::header_put::StorageBlocksGenesisInitHeaderPutErrorAction,
    ),
    TestStorageBlocksGenesisInitHeaderPutSuccessAction(
        storage::blocks::genesis::init::header_put::StorageBlocksGenesisInitHeaderPutSuccessAction,
    ),
    TestStorageBlocksGenesisInitAdditionalDataPutInitAction(
        storage::blocks::genesis::init::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutInitAction,
    ),
    TestStorageBlocksGenesisInitAdditionalDataPutPendingAction(
        storage::blocks::genesis::init::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutPendingAction,
    ),
    TestStorageBlocksGenesisInitAdditionalDataPutErrorAction(
        storage::blocks::genesis::init::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutErrorAction,
    ),
    TestStorageBlocksGenesisInitAdditionalDataPutSuccessAction(
        storage::blocks::genesis::init::additional_data_put::StorageBlocksGenesisInitAdditionalDataPutSuccessAction,
    ),
    TestStorageBlocksGenesisInitCommitResultGetInitAction(
        storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetInitAction,
    ),
    TestStorageBlocksGenesisInitCommitResultGetPendingAction(
        storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetPendingAction,
    ),
    TestStorageBlocksGenesisInitCommitResultGetErrorAction(
        storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetErrorAction,
    ),
    TestStorageBlocksGenesisInitCommitResultGetSuccessAction(
        storage::blocks::genesis::init::commit_result_get::StorageBlocksGenesisInitCommitResultGetSuccessAction,
    ),
    TestStorageBlocksGenesisInitCommitResultPutInitAction(
        storage::blocks::genesis::init::commit_result_put::StorageBlocksGenesisInitCommitResultPutInitAction,
    ),
    TestStorageBlocksGenesisInitCommitResultPutErrorAction(
        storage::blocks::genesis::init::commit_result_put::StorageBlocksGenesisInitCommitResultPutErrorAction,
    ),
    TestStorageBlocksGenesisInitCommitResultPutSuccessAction(
        storage::blocks::genesis::init::commit_result_put::StorageBlocksGenesisInitCommitResultPutSuccessAction,
    ),
    TestStorageBlocksGenesisInitSuccessAction(
        storage::blocks::genesis::init::StorageBlocksGenesisInitSuccessAction,
    ),
}

impl StorageActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestStorageRequestInitAction(a) => a.into(),
            Self::TestStorageRequestPendingAction(a) => a.into(),
            Self::TestStorageResponseReceivedAction(a) => a.into(),
            Self::TestStorageRequestErrorAction(a) => a.into(),
            Self::TestStorageRequestSuccessAction(a) => a.into(),
            Self::TestStorageRequestFinishAction(a) => a.into(),
            Self::TestStorageStateSnapshotCreateInitAction(a) => a.into(),
            Self::TestStorageStateSnapshotCreatePendingAction(a) => a.into(),
            Self::TestStorageStateSnapshotCreateErrorAction(a) => a.into(),
            Self::TestStorageStateSnapshotCreateSuccessAction(a) => a.into(),
            /*
            Self::TestStorageBlockHeaderGetAction(a) => a.into(),
            Self::TestStorageBlockHeaderOkAction(a) => a.into(),
            Self::TestStorageBlockHeaderErrorAction(a) => a.into(),
            Self::TestStorageBlockMetaGetAction(a) => a.into(),
            Self::TestStorageBlockMetaOkAction(a) => a.into(),
            Self::TestStorageBlockMetaErrorAction(a) => a.into(),
            Self::TestStorageOperationsGetAction(a) => a.into(),
            Self::TestStorageOperationsOkAction(a) => a.into(),
            Self::TestStorageOperationsErrorAction(a) => a.into(),
            Self::TestStorageBlockAdditionalDataGetAction(a) => a.into(),
            Self::TestStorageBlockAdditionalDataOkAction(a) => a.into(),
            Self::TestStorageBlockAdditionalDataErrorAction(a) => a.into(),
            Self::TestStorageConstantsGetAction(a) => a.into(),
            Self::TestStorageConstantsOkAction(a) => a.into(),
            Self::TestStorageConstantsErrorAction(a) => a.into(),
            Self::TestStorageCycleMetaGetAction(a) => a.into(),
            Self::TestStorageCycleMetaOkAction(a) => a.into(),
            Self::TestStorageCycleMetaErrorAction(a) => a.into(),
            Self::TestStorageCycleErasGetAction(a) => a.into(),
            Self::TestStorageCycleErasOkAction(a) => a.into(),
            Self::TestStorageCycleErasErrorAction(a) => a.into(),
            */
            Self::TestStorageBlocksGenesisCheckAppliedInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisCheckAppliedGetMetaPendingAction(a) => a.into(),
            Self::TestStorageBlocksGenesisCheckAppliedGetMetaErrorAction(a) => a.into(),
            Self::TestStorageBlocksGenesisCheckAppliedGetMetaSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisCheckAppliedSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitHeaderPutInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitHeaderPutPendingAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitHeaderPutErrorAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitHeaderPutSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitAdditionalDataPutInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitAdditionalDataPutPendingAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitAdditionalDataPutErrorAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitAdditionalDataPutSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultGetInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultGetPendingAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultGetErrorAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultGetSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultPutInitAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultPutErrorAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitCommitResultPutSuccessAction(a) => a.into(),
            Self::TestStorageBlocksGenesisInitSuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum MempoolActionTest {
    TestMempoolRecvDoneAction(mempool_actions::MempoolRecvDoneAction),
    TestMempoolGetOperationsAction(mempool_actions::MempoolGetOperationsAction),
    TestMempoolMarkOperationsAsPendingAction(mempool_actions::MempoolMarkOperationsAsPendingAction),
    TestMempoolOperationRecvDoneAction(mempool_actions::MempoolOperationRecvDoneAction),
    TestMempoolOperationInjectAction(mempool_actions::MempoolOperationInjectAction),
    TestMempoolValidateStartAction(mempool_actions::MempoolValidateStartAction),
    TestMempoolValidateWaitPrevalidatorAction(
        mempool_actions::MempoolValidateWaitPrevalidatorAction,
    ),
    TestMempoolRpcRespondAction(mempool_actions::MempoolRpcRespondAction),
    TestMempoolRegisterOperationsStreamAction(
        mempool_actions::MempoolRegisterOperationsStreamAction,
    ),
    TestMempoolUnregisterOperationsStreamsAction(
        mempool_actions::MempoolUnregisterOperationsStreamsAction,
    ),
    TestMempoolSendAction(mempool_actions::MempoolSendAction),
    TestMempoolSendValidatedAction(mempool_actions::MempoolSendValidatedAction),
    TestMempoolAskCurrentHeadAction(mempool_actions::MempoolAskCurrentHeadAction),
    TestMempoolBroadcastAction(mempool_actions::MempoolBroadcastAction),
    TestMempoolBroadcastDoneAction(mempool_actions::MempoolBroadcastDoneAction),
    TestMempoolCleanupWaitPrevalidatorAction(mempool_actions::MempoolCleanupWaitPrevalidatorAction),
    TestMempoolRemoveAppliedOperationsAction(mempool_actions::MempoolRemoveAppliedOperationsAction),
    TestMempoolGetPendingOperationsAction(mempool_actions::MempoolGetPendingOperationsAction),
    TestMempoolFlushAction(mempool_actions::MempoolFlushAction),
    TestMempoolOperationDecodedAction(mempool_actions::MempoolOperationDecodedAction),
    TestMempoolRpcEndorsementsStatusGetAction(
        mempool_actions::MempoolRpcEndorsementsStatusGetAction,
    ),
}

impl MempoolActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestMempoolRecvDoneAction(a) => a.into(),
            Self::TestMempoolGetOperationsAction(a) => a.into(),
            Self::TestMempoolMarkOperationsAsPendingAction(a) => a.into(),
            Self::TestMempoolOperationRecvDoneAction(a) => a.into(),
            Self::TestMempoolOperationInjectAction(a) => a.into(),
            Self::TestMempoolValidateStartAction(a) => a.into(),
            Self::TestMempoolValidateWaitPrevalidatorAction(a) => a.into(),
            Self::TestMempoolRpcRespondAction(a) => a.into(),
            Self::TestMempoolRegisterOperationsStreamAction(a) => a.into(),
            Self::TestMempoolUnregisterOperationsStreamsAction(a) => a.into(),
            Self::TestMempoolSendAction(a) => a.into(),
            Self::TestMempoolSendValidatedAction(a) => a.into(),
            Self::TestMempoolAskCurrentHeadAction(a) => a.into(),
            Self::TestMempoolBroadcastAction(a) => a.into(),
            Self::TestMempoolBroadcastDoneAction(a) => a.into(),
            Self::TestMempoolCleanupWaitPrevalidatorAction(a) => a.into(),
            Self::TestMempoolRemoveAppliedOperationsAction(a) => a.into(),
            Self::TestMempoolGetPendingOperationsAction(a) => a.into(),
            Self::TestMempoolFlushAction(a) => a.into(),
            Self::TestMempoolOperationDecodedAction(a) => a.into(),
            Self::TestMempoolRpcEndorsementsStatusGetAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PrecheckerActionTest {
    TestPrecheckerPrecheckOperationRequestAction(
        prechecker_actions::PrecheckerPrecheckOperationRequestAction,
    ),
    TestPrecheckerPrecheckOperationResponseAction(
        prechecker_actions::PrecheckerPrecheckOperationResponseAction,
    ),
    TestPrecheckerCacheAppliedBlockAction(prechecker_actions::PrecheckerCacheAppliedBlockAction),
    TestPrecheckerPrecheckOperationInitAction(
        prechecker_actions::PrecheckerPrecheckOperationInitAction,
    ),
    TestPrecheckerGetProtocolVersionAction(prechecker_actions::PrecheckerGetProtocolVersionAction),
    TestPrecheckerProtocolVersionReadyAction(
        prechecker_actions::PrecheckerProtocolVersionReadyAction,
    ),
    TestPrecheckerDecodeOperationAction(prechecker_actions::PrecheckerDecodeOperationAction),
    TestPrecheckerOperationDecodedAction(prechecker_actions::PrecheckerOperationDecodedAction),
    //TestPrecheckerWaitForBlockApplicationAction(
    //    prechecker_actions::PrecheckerWaitForBlockApplicationAction,
    //),
    TestPrecheckerWaitForBlockPrecheckedAction(
        prechecker_actions::PrecheckerWaitForBlockPrecheckedAction,
    ),
    TestPrecheckerBlockPrecheckedAction(prechecker_actions::PrecheckerBlockPrecheckedAction),
    TestPrecheckerWaitForBlockApplied(prechecker_actions::PrecheckerWaitForBlockAppliedAction),
    TestPrecheckerBlockAppliedAction(prechecker_actions::PrecheckerBlockAppliedAction),
    TestPrecheckerGetEndorsingRightsAction(prechecker_actions::PrecheckerGetEndorsingRightsAction),
    TestPrecheckerEndorsingRightsReadyAction(
        prechecker_actions::PrecheckerEndorsingRightsReadyAction,
    ),
    TestPrecheckerValidateEndorsementAction(
        prechecker_actions::PrecheckerValidateEndorsementAction,
    ),
    TestPrecheckerEndorsementValidationAppliedAction(
        prechecker_actions::PrecheckerEndorsementValidationAppliedAction,
    ),
    TestPrecheckerEndorsementValidationRefusedAction(
        prechecker_actions::PrecheckerEndorsementValidationRefusedAction,
    ),
    TestPrecheckerProtocolNeededAction(prechecker_actions::PrecheckerProtocolNeededAction),
    TestPrecheckerErrorAction(prechecker_actions::PrecheckerErrorAction),
    TestPrecheckerPrecacheEndorsingRightsAction(
        prechecker_actions::PrecheckerPrecacheEndorsingRightsAction,
    ),
    TestPrecheckerSetNextBlockProtocolAction(
        prechecker_actions::PrecheckerSetNextBlockProtocolAction,
    ),
    TestPrecheckerQueryNextBlockProtocolAction(
        prechecker_actions::PrecheckerQueryNextBlockProtocolAction,
    ),
    TestPrecheckerNextBlockProtocolReadyAction(
        prechecker_actions::PrecheckerNextBlockProtocolReadyAction,
    ),
    TestPrecheckerNextBlockProtocolErrorAction(
        prechecker_actions::PrecheckerNextBlockProtocolErrorAction,
    ),
    TestPrecheckerPruneOperationAction(prechecker_actions::PrecheckerPruneOperationAction),
}

impl PrecheckerActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPrecheckerPrecheckOperationRequestAction(a) => a.into(),
            Self::TestPrecheckerPrecheckOperationResponseAction(a) => a.into(),
            Self::TestPrecheckerCacheAppliedBlockAction(a) => a.into(),
            Self::TestPrecheckerPrecheckOperationInitAction(a) => a.into(),
            Self::TestPrecheckerGetProtocolVersionAction(a) => a.into(),
            Self::TestPrecheckerProtocolVersionReadyAction(a) => a.into(),
            Self::TestPrecheckerDecodeOperationAction(a) => a.into(),
            Self::TestPrecheckerOperationDecodedAction(a) => a.into(),
            //Self::TestPrecheckerWaitForBlockApplicationAction(a) => a.into(),
            Self::TestPrecheckerWaitForBlockPrecheckedAction(a) => a.into(),
            Self::TestPrecheckerBlockPrecheckedAction(a) => a.into(),
            Self::TestPrecheckerWaitForBlockApplied(a) => a.into(),
            Self::TestPrecheckerBlockAppliedAction(a) => a.into(),
            Self::TestPrecheckerGetEndorsingRightsAction(a) => a.into(),
            Self::TestPrecheckerEndorsingRightsReadyAction(a) => a.into(),
            Self::TestPrecheckerValidateEndorsementAction(a) => a.into(),
            Self::TestPrecheckerEndorsementValidationAppliedAction(a) => a.into(),
            Self::TestPrecheckerEndorsementValidationRefusedAction(a) => a.into(),
            Self::TestPrecheckerProtocolNeededAction(a) => a.into(),
            Self::TestPrecheckerErrorAction(a) => a.into(),
            Self::TestPrecheckerPrecacheEndorsingRightsAction(a) => a.into(),
            Self::TestPrecheckerSetNextBlockProtocolAction(a) => a.into(),
            Self::TestPrecheckerQueryNextBlockProtocolAction(a) => a.into(),
            Self::TestPrecheckerNextBlockProtocolReadyAction(a) => a.into(),
            Self::TestPrecheckerNextBlockProtocolErrorAction(a) => a.into(),
            Self::TestPrecheckerPruneOperationAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerActionTest {
    TestPeersGraylistAction(PeersGraylistActionTest),
    TestPeersAddMultiAction(PeersAddMultiAction),
    TestPeersRemoveAction(PeersRemoveAction),
    TestPeerConnection(PeerConnectionActionTest),
    TestPeerChunking(PeerChunkActionTest),
    TestPeerMessages(PeerMessageActionTest),
    TestPeerHandshaking(PeerHandshakingActionTest),
}

impl PeerActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersGraylistAction(a) => a.to_action(),
            Self::TestPeersAddMultiAction(a) => a.into(),
            Self::TestPeersRemoveAction(a) => a.into(),
            Self::TestPeerConnection(a) => a.to_action(),
            Self::TestPeerChunking(a) => a.to_action(),
            Self::TestPeerMessages(a) => a.to_action(),
            Self::TestPeerHandshaking(a) => a.to_action(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerConnectionActionTest {
    TestPeersTimeout(PeersTimeoutActionTest),
    TestPeerIncomingConnection(PeerIncomingConnectionActionTest),
    TestPeerOutgoingConnection(PeerOutgoingConnectionActionTest),
    TestPeerConnectionClosedAction(PeerConnectionClosedAction),
    TestPeerDisconnectAction(PeerDisconnectAction),
    TestPeerDisconnectedAction(PeerDisconnectedAction),
}

impl PeerConnectionActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersTimeout(a) => a.to_action(),
            Self::TestPeerIncomingConnection(a) => a.to_action(),
            Self::TestPeerOutgoingConnection(a) => a.to_action(),
            Self::TestPeerConnectionClosedAction(a) => a.into(),
            Self::TestPeerDisconnectAction(a) => a.into(),
            Self::TestPeerDisconnectedAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum ControlActionTest {
    TestInitAction(InitAction),
    TestPeersInitAction(PeersInitAction),
    TestMioWaitForEventsAction(MioWaitForEventsAction),
    TestMioTimeoutEvent(MioTimeoutEvent),
    TestP2pServerEvent(P2pServerEvent),
    TestP2pPeerEvent(P2pPeerEvent),
    TestWakeupEvent(WakeupEvent),
    TestProtocolAction(ProtocolAction),
    TestShutdownInitAction(ShutdownInitAction),
    TestShutdownPendingAction(ShutdownPendingAction),
    TestShutdownSuccessAction(ShutdownSuccessAction),
    TestPausedLoopsAddAction(PausedLoopsAddAction),
    TestPausedLoopsResumeAllAction(PausedLoopsResumeAllAction),
    TestPausedLoopsResumeNextInitAction(PausedLoopsResumeNextInitAction),
    TestPausedLoopsResumeNextSuccessAction(PausedLoopsResumeNextSuccessAction),
    TestPeerTryWriteLoopStartAction(PeerTryWriteLoopStartAction),
    TestPeerTryWriteLoopFinishAction(PeerTryWriteLoopFinishAction),
    TestPeerTryReadLoopStartAction(PeerTryReadLoopStartAction),
    TestPeerTryReadLoopFinishAction(PeerTryReadLoopFinishAction),
}

impl ControlActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestInitAction(a) => a.into(),
            Self::TestPeersInitAction(a) => a.into(),
            Self::TestMioWaitForEventsAction(a) => a.into(),
            Self::TestMioTimeoutEvent(a) => a.into(),
            Self::TestP2pServerEvent(a) => a.into(),
            Self::TestP2pPeerEvent(a) => a.into(),
            Self::TestWakeupEvent(a) => a.into(),
            Self::TestProtocolAction(a) => a.into(),
            Self::TestShutdownInitAction(a) => a.into(),
            Self::TestShutdownPendingAction(a) => a.into(),
            Self::TestShutdownSuccessAction(a) => a.into(),
            Self::TestPausedLoopsAddAction(a) => a.into(),
            Self::TestPausedLoopsResumeAllAction(a) => a.into(),
            Self::TestPausedLoopsResumeNextInitAction(a) => a.into(),
            Self::TestPausedLoopsResumeNextSuccessAction(a) => a.into(),
            Self::TestPeerTryWriteLoopStartAction(a) => a.into(),
            Self::TestPeerTryWriteLoopFinishAction(a) => a.into(),
            Self::TestPeerTryReadLoopStartAction(a) => a.into(),
            Self::TestPeerTryReadLoopFinishAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeersDnsLookupActionTest {
    TestPeersDnsLookupInitAction(PeersDnsLookupInitAction),
    TestPeersDnsLookupErrorAction(PeersDnsLookupErrorAction),
    TestPeersDnsLookupSuccessAction(PeersDnsLookupSuccessAction),
    TestPeersDnsLookupCleanupAction(PeersDnsLookupCleanupAction),
}

impl PeersDnsLookupActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersDnsLookupInitAction(a) => a.into(),
            Self::TestPeersDnsLookupErrorAction(a) => a.into(),
            Self::TestPeersDnsLookupSuccessAction(a) => a.into(),
            Self::TestPeersDnsLookupCleanupAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeersGraylistActionTest {
    TestPeersGraylistAddressAction(PeersGraylistAddressAction),
    TestPeersGraylistIpAddAction(PeersGraylistIpAddAction),
    TestPeersGraylistIpAddedAction(PeersGraylistIpAddedAction),
    TestPeersGraylistIpRemoveAction(PeersGraylistIpRemoveAction),
    TestPeersGraylistIpRemovedAction(PeersGraylistIpRemovedAction),
}

impl PeersGraylistActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersGraylistAddressAction(a) => a.into(),
            Self::TestPeersGraylistIpAddAction(a) => a.into(),
            Self::TestPeersGraylistIpAddedAction(a) => a.into(),
            Self::TestPeersGraylistIpRemoveAction(a) => a.into(),
            Self::TestPeersGraylistIpRemovedAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeersTimeoutActionTest {
    TestPeersCheckTimeoutsInitAction(PeersCheckTimeoutsInitAction),
    TestPeersCheckTimeoutsSuccessAction(PeersCheckTimeoutsSuccessAction),
    TestPeersCheckTimeoutsCleanupAction(PeersCheckTimeoutsCleanupAction),
}

impl PeersTimeoutActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersCheckTimeoutsInitAction(a) => a.into(),
            Self::TestPeersCheckTimeoutsSuccessAction(a) => a.into(),
            Self::TestPeersCheckTimeoutsCleanupAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerIncomingConnectionActionTest {
    TestPeersAddIncomingPeerAction(PeersAddIncomingPeerAction),
    TestPeerConnectionIncomingAcceptAction(PeerConnectionIncomingAcceptAction),
    TestPeerConnectionIncomingAcceptErrorAction(PeerConnectionIncomingAcceptErrorAction),
    TestPeerConnectionIncomingRejectedAction(PeerConnectionIncomingRejectedAction),
    TestPeerConnectionIncomingAcceptSuccessAction(PeerConnectionIncomingAcceptSuccessAction),
    TestPeerConnectionIncomingErrorAction(PeerConnectionIncomingErrorAction),
    TestPeerConnectionIncomingSuccessAction(PeerConnectionIncomingSuccessAction),
}

impl PeerIncomingConnectionActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeersAddIncomingPeerAction(a) => a.into(),
            Self::TestPeerConnectionIncomingAcceptAction(a) => a.into(),
            Self::TestPeerConnectionIncomingAcceptErrorAction(a) => a.into(),
            Self::TestPeerConnectionIncomingRejectedAction(a) => a.into(),
            Self::TestPeerConnectionIncomingAcceptSuccessAction(a) => a.into(),
            Self::TestPeerConnectionIncomingErrorAction(a) => a.into(),
            Self::TestPeerConnectionIncomingSuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerOutgoingConnectionActionTest {
    TestPeerConnectionOutgoingRandomInitAction(PeerConnectionOutgoingRandomInitAction),
    TestPeerConnectionOutgoingInitAction(PeerConnectionOutgoingInitAction),
    TestPeerConnectionOutgoingPendingAction(PeerConnectionOutgoingPendingAction),
    TestPeerConnectionOutgoingErrorAction(PeerConnectionOutgoingErrorAction),
    TestPeerConnectionOutgoingSuccessAction(PeerConnectionOutgoingSuccessAction),
}

impl PeerOutgoingConnectionActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeerConnectionOutgoingRandomInitAction(a) => a.into(),
            Self::TestPeerConnectionOutgoingInitAction(a) => a.into(),
            Self::TestPeerConnectionOutgoingPendingAction(a) => a.into(),
            Self::TestPeerConnectionOutgoingErrorAction(a) => a.into(),
            Self::TestPeerConnectionOutgoingSuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerChunkActionTest {
    TestPeerChunkReadInitAction(PeerChunkReadInitAction),
    TestPeerChunkReadPartAction(PeerChunkReadPartAction),
    TestPeerChunkReadDecryptAction(PeerChunkReadDecryptAction),
    TestPeerChunkReadReadyAction(PeerChunkReadReadyAction),
    TestPeerChunkReadErrorAction(PeerChunkReadErrorAction),
    TestPeerChunkWriteSetContentAction(PeerChunkWriteSetContentAction),
    TestPeerChunkWriteEncryptContentAction(PeerChunkWriteEncryptContentAction),
    TestPeerChunkWriteCreateChunkAction(PeerChunkWriteCreateChunkAction),
    TestPeerChunkWritePartAction(PeerChunkWritePartAction),
    TestPeerChunkWriteReadyAction(PeerChunkWriteReadyAction),
    TestPeerChunkWriteErrorAction(PeerChunkWriteErrorAction),
    TestPeerBinaryMessageReadInitAction(PeerBinaryMessageReadInitAction),
    TestPeerBinaryMessageReadChunkReadyAction(PeerBinaryMessageReadChunkReadyAction),
    TestPeerBinaryMessageReadSizeReadyAction(PeerBinaryMessageReadSizeReadyAction),
    TestPeerBinaryMessageReadReadyAction(PeerBinaryMessageReadReadyAction),
    TestPeerBinaryMessageReadErrorAction(PeerBinaryMessageReadErrorAction),
    TestPeerBinaryMessageWriteSetContentAction(PeerBinaryMessageWriteSetContentAction),
    TestPeerBinaryMessageWriteNextChunkAction(PeerBinaryMessageWriteNextChunkAction),
    TestPeerBinaryMessageWriteReadyAction(PeerBinaryMessageWriteReadyAction),
    TestPeerBinaryMessageWriteErrorAction(PeerBinaryMessageWriteErrorAction),
}

impl PeerChunkActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeerChunkReadInitAction(a) => a.into(),
            Self::TestPeerChunkReadPartAction(a) => a.into(),
            Self::TestPeerChunkReadDecryptAction(a) => a.into(),
            Self::TestPeerChunkReadReadyAction(a) => a.into(),
            Self::TestPeerChunkReadErrorAction(a) => a.into(),
            Self::TestPeerChunkWriteSetContentAction(a) => a.into(),
            Self::TestPeerChunkWriteEncryptContentAction(a) => a.into(),
            Self::TestPeerChunkWriteCreateChunkAction(a) => a.into(),
            Self::TestPeerChunkWritePartAction(a) => a.into(),
            Self::TestPeerChunkWriteReadyAction(a) => a.into(),
            Self::TestPeerChunkWriteErrorAction(a) => a.into(),
            Self::TestPeerBinaryMessageReadInitAction(a) => a.into(),
            Self::TestPeerBinaryMessageReadChunkReadyAction(a) => a.into(),
            Self::TestPeerBinaryMessageReadSizeReadyAction(a) => a.into(),
            Self::TestPeerBinaryMessageReadReadyAction(a) => a.into(),
            Self::TestPeerBinaryMessageReadErrorAction(a) => a.into(),
            Self::TestPeerBinaryMessageWriteSetContentAction(a) => a.into(),
            Self::TestPeerBinaryMessageWriteNextChunkAction(a) => a.into(),
            Self::TestPeerBinaryMessageWriteReadyAction(a) => a.into(),
            Self::TestPeerBinaryMessageWriteErrorAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerMessageActionTest {
    TestPeerMessageReadInitAction(PeerMessageReadInitAction),
    TestPeerMessageReadErrorAction(PeerMessageReadErrorAction),
    TestPeerMessageReadSuccessAction(PeerMessageReadSuccessAction),
    TestPeerMessageWriteNextAction(PeerMessageWriteNextAction),
    TestPeerMessageWriteInitAction(PeerMessageWriteInitAction),
    TestPeerMessageWriteErrorAction(PeerMessageWriteErrorAction),
    TestPeerMessageWriteSuccessAction(PeerMessageWriteSuccessAction),
}

impl PeerMessageActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeerMessageReadInitAction(a) => a.into(),
            Self::TestPeerMessageReadErrorAction(a) => a.into(),
            Self::TestPeerMessageReadSuccessAction(a) => a.into(),
            Self::TestPeerMessageWriteNextAction(a) => a.into(),
            Self::TestPeerMessageWriteInitAction(a) => a.into(),
            Self::TestPeerMessageWriteErrorAction(a) => a.into(),
            Self::TestPeerMessageWriteSuccessAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerHandshakingActionTest {
    TestPeerHandshakingInitAction(PeerHandshakingInitAction),
    TestPeerHandshakingConnectionMessageInitAction(PeerHandshakingConnectionMessageInitAction),
    TestPeerHandshakingConnectionMessageEncodeAction(PeerHandshakingConnectionMessageEncodeAction),
    TestPeerHandshakingConnectionMessageWriteAction(PeerHandshakingConnectionMessageWriteAction),
    TestPeerHandshakingConnectionMessageReadAction(PeerHandshakingConnectionMessageReadAction),
    TestPeerHandshakingConnectionMessageDecodeAction(PeerHandshakingConnectionMessageDecodeAction),
    TestPeerHandshakingEncryptionInitAction(PeerHandshakingEncryptionInitAction),
    TestPeerHandshakingMetadataMessageInitAction(PeerHandshakingMetadataMessageInitAction),
    TestPeerHandshakingMetadataMessageEncodeAction(PeerHandshakingMetadataMessageEncodeAction),
    TestPeerHandshakingMetadataMessageWriteAction(PeerHandshakingMetadataMessageWriteAction),
    TestPeerHandshakingMetadataMessageReadAction(PeerHandshakingMetadataMessageReadAction),
    TestPeerHandshakingMetadataMessageDecodeAction(PeerHandshakingMetadataMessageDecodeAction),
    TestPeerHandshakingAckMessageInitAction(PeerHandshakingAckMessageInitAction),
    TestPeerHandshakingAckMessageEncodeAction(PeerHandshakingAckMessageEncodeAction),
    TestPeerHandshakingAckMessageWriteAction(PeerHandshakingAckMessageWriteAction),
    TestPeerHandshakingAckMessageReadAction(PeerHandshakingAckMessageReadAction),
    TestPeerHandshakingAckMessageDecodeAction(PeerHandshakingAckMessageDecodeAction),
    TestPeerHandshakingErrorAction(PeerHandshakingErrorAction),
    TestPeerHandshakingFinishAction(PeerHandshakingFinishAction),
}

impl PeerHandshakingActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeerHandshakingInitAction(a) => a.into(),
            Self::TestPeerHandshakingConnectionMessageInitAction(a) => a.into(),
            Self::TestPeerHandshakingConnectionMessageEncodeAction(a) => a.into(),
            Self::TestPeerHandshakingConnectionMessageWriteAction(a) => a.into(),
            Self::TestPeerHandshakingConnectionMessageReadAction(a) => a.into(),
            Self::TestPeerHandshakingConnectionMessageDecodeAction(a) => a.into(),
            Self::TestPeerHandshakingEncryptionInitAction(a) => a.into(),
            Self::TestPeerHandshakingMetadataMessageInitAction(a) => a.into(),
            Self::TestPeerHandshakingMetadataMessageEncodeAction(a) => a.into(),
            Self::TestPeerHandshakingMetadataMessageWriteAction(a) => a.into(),
            Self::TestPeerHandshakingMetadataMessageReadAction(a) => a.into(),
            Self::TestPeerHandshakingMetadataMessageDecodeAction(a) => a.into(),
            Self::TestPeerHandshakingAckMessageInitAction(a) => a.into(),
            Self::TestPeerHandshakingAckMessageEncodeAction(a) => a.into(),
            Self::TestPeerHandshakingAckMessageWriteAction(a) => a.into(),
            Self::TestPeerHandshakingAckMessageReadAction(a) => a.into(),
            Self::TestPeerHandshakingAckMessageDecodeAction(a) => a.into(),
            Self::TestPeerHandshakingErrorAction(a) => a.into(),
            Self::TestPeerHandshakingFinishAction(a) => a.into(),
        }
    }
}

fn next_action_id() -> ActionId {
    let time_ns = FUZZER_STATE
        .read()
        .unwrap()
        .current_target_state
        .last_action
        .time_as_nanos();
    ActionId::new_unchecked(time_ns + 1)
}

fn reduce_with_state(action: &ActionWithMeta<Action>) -> bool {
    let mut state = FUZZER_STATE.write().unwrap();

    if state.reset_count != 0 && (state.iteration_count % state.reset_count) == 0 {
        //println!("Resetting state: iteration {}", state.iteration_count);
        state.current_target_state = state.initial_target_state.clone();
    }

    state.iteration_count += 1;

    if !is_action_enabled(action.action.clone(), &state.current_target_state) {
        return true;
    }

    //println!("{:?}", action);
    shell_automaton::reducer(&mut state.current_target_state, action);
    match state.current_target_state.check_safety_condition() {
        Err(error) => {
            println!("{:?}", error);
            false
        }
        _ => true,
    }
}

fn action_test_all(action_test: &AllActionsTest) -> bool {
    let action = ActionWithMeta {
        id: next_action_id(),
        depth: 0,
        action: action_test.to_action(),
    };

    reduce_with_state(&action)
}

fn action_test_control(action_test: &ControlActionTest) -> bool {
    let action = ActionWithMeta {
        id: next_action_id(),
        depth: 0,
        action: action_test.to_action(),
    };

    reduce_with_state(&action)
}

fn action_test_dns(action_test: &PeersDnsLookupActionTest) -> bool {
    let action = ActionWithMeta {
        id: next_action_id(),
        depth: 0,
        action: action_test.to_action(),
    };

    reduce_with_state(&action)
}

fn action_test_peer(action_test: &PeerActionTest) -> bool {
    let action = ActionWithMeta {
        id: next_action_id(),
        depth: 0,
        action: action_test.to_action(),
    };

    reduce_with_state(&action)
}

fn action_test_storage(action_test: &StorageActionTest) -> bool {
    let action = ActionWithMeta {
        id: next_action_id(),
        depth: 0,
        action: action_test.to_action(),
    };

    reduce_with_state(&action)
}

/*
    TODO: we can't take advantage of enum_dispatch because the EnablingCondition trait
    is defined in redux-rs, while Action is in shell_automaton. A solution would be
    to move the redux-rs implementation inside shell_automaton.
*/
fn is_action_enabled(action: Action, state: &State) -> bool {
    match action {
        Action::Init(a) => a.is_enabled(state),
        Action::PausedLoopsAdd(a) => a.is_enabled(state),
        Action::PausedLoopsResumeAll(a) => a.is_enabled(state),
        Action::PausedLoopsResumeNextInit(a) => a.is_enabled(state),
        Action::PausedLoopsResumeNextSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerStart(a) => a.is_enabled(state),
        Action::ProtocolRunnerSpawnServerInit(a) => a.is_enabled(state),
        Action::ProtocolRunnerSpawnServerPending(a) => a.is_enabled(state),
        Action::ProtocolRunnerSpawnServerError(a) => a.is_enabled(state),
        Action::ProtocolRunnerSpawnServerSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerInit(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitRuntime(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitRuntimePending(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitRuntimeError(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitRuntimeSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitCheckGenesisApplied(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitCheckGenesisAppliedSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContext(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextPending(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextError(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextIpcServer(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextIpcServerPending(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextIpcServerError(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitContextIpcServerSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerInitSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerReady(a) => a.is_enabled(state),
        Action::ProtocolRunnerNotifyStatus(a) => a.is_enabled(state),
        Action::ProtocolRunnerResponse(a) => a.is_enabled(state),
        Action::ProtocolRunnerResponseUnexpected(a) => a.is_enabled(state),
        Action::BlockApplierEnqueueBlock(a) => a.is_enabled(state),
        Action::BlockApplierApplyInit(a) => a.is_enabled(state),
        Action::BlockApplierApplyPrepareDataPending(a) => a.is_enabled(state),
        Action::BlockApplierApplyPrepareDataSuccess(a) => a.is_enabled(state),
        Action::BlockApplierApplyProtocolRunnerApplyInit(a) => a.is_enabled(state),
        Action::BlockApplierApplyProtocolRunnerApplyPending(a) => a.is_enabled(state),
        Action::BlockApplierApplyProtocolRunnerApplyRetry(a) => a.is_enabled(state),
        Action::BlockApplierApplyProtocolRunnerApplySuccess(a) => a.is_enabled(state),
        Action::BlockApplierApplyStoreApplyResultPending(a) => a.is_enabled(state),
        Action::BlockApplierApplyStoreApplyResultSuccess(a) => a.is_enabled(state),
        Action::BlockApplierApplyError(a) => a.is_enabled(state),
        Action::BlockApplierApplySuccess(a) => a.is_enabled(state),
        Action::PeersInit(a) => a.is_enabled(state),
        Action::PeersDnsLookupInit(a) => a.is_enabled(state),
        Action::PeersDnsLookupError(a) => a.is_enabled(state),
        Action::PeersDnsLookupSuccess(a) => a.is_enabled(state),
        Action::PeersDnsLookupCleanup(a) => a.is_enabled(state),
        Action::PeersGraylistAddress(a) => a.is_enabled(state),
        Action::PeersGraylistIpAdd(a) => a.is_enabled(state),
        Action::PeersGraylistIpAdded(a) => a.is_enabled(state),
        Action::PeersGraylistIpRemove(a) => a.is_enabled(state),
        Action::PeersGraylistIpRemoved(a) => a.is_enabled(state),
        Action::PeersAddIncomingPeer(a) => a.is_enabled(state),
        Action::PeersAddMulti(a) => a.is_enabled(state),
        Action::PeersRemove(a) => a.is_enabled(state),
        Action::PeersCheckTimeoutsInit(a) => a.is_enabled(state),
        Action::PeersCheckTimeoutsSuccess(a) => a.is_enabled(state),
        Action::PeersCheckTimeoutsCleanup(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingAccept(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingAcceptError(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingRejected(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingAcceptSuccess(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingError(a) => a.is_enabled(state),
        Action::PeerConnectionIncomingSuccess(a) => a.is_enabled(state),
        Action::PeerConnectionOutgoingRandomInit(a) => a.is_enabled(state),
        Action::PeerConnectionOutgoingInit(a) => a.is_enabled(state),
        Action::PeerConnectionOutgoingPending(a) => a.is_enabled(state),
        Action::PeerConnectionOutgoingError(a) => a.is_enabled(state),
        Action::PeerConnectionOutgoingSuccess(a) => a.is_enabled(state),
        Action::PeerConnectionClosed(a) => a.is_enabled(state),
        Action::PeerDisconnect(a) => a.is_enabled(state),
        Action::PeerDisconnected(a) => a.is_enabled(state),
        Action::MioWaitForEvents(a) => a.is_enabled(state),
        Action::MioTimeoutEvent(a) => a.is_enabled(state),
        Action::P2pServerEvent(a) => a.is_enabled(state),
        Action::P2pPeerEvent(a) => a.is_enabled(state),
        Action::WakeupEvent(a) => a.is_enabled(state),
        Action::PeerTryWriteLoopStart(a) => a.is_enabled(state),
        Action::PeerTryWriteLoopFinish(a) => a.is_enabled(state),
        Action::PeerTryReadLoopStart(a) => a.is_enabled(state),
        Action::PeerTryReadLoopFinish(a) => a.is_enabled(state),
        Action::PeerChunkReadInit(a) => a.is_enabled(state),
        Action::PeerChunkReadPart(a) => a.is_enabled(state),
        Action::PeerChunkReadDecrypt(a) => a.is_enabled(state),
        Action::PeerChunkReadReady(a) => a.is_enabled(state),
        Action::PeerChunkReadError(a) => a.is_enabled(state),
        Action::PeerChunkWriteSetContent(a) => a.is_enabled(state),
        Action::PeerChunkWriteEncryptContent(a) => a.is_enabled(state),
        Action::PeerChunkWriteCreateChunk(a) => a.is_enabled(state),
        Action::PeerChunkWritePart(a) => a.is_enabled(state),
        Action::PeerChunkWriteReady(a) => a.is_enabled(state),
        Action::PeerChunkWriteError(a) => a.is_enabled(state),
        Action::PeerBinaryMessageReadInit(a) => a.is_enabled(state),
        Action::PeerBinaryMessageReadChunkReady(a) => a.is_enabled(state),
        Action::PeerBinaryMessageReadSizeReady(a) => a.is_enabled(state),
        Action::PeerBinaryMessageReadReady(a) => a.is_enabled(state),
        Action::PeerBinaryMessageReadError(a) => a.is_enabled(state),
        Action::PeerBinaryMessageWriteSetContent(a) => a.is_enabled(state),
        Action::PeerBinaryMessageWriteNextChunk(a) => a.is_enabled(state),
        Action::PeerBinaryMessageWriteReady(a) => a.is_enabled(state),
        Action::PeerBinaryMessageWriteError(a) => a.is_enabled(state),
        Action::PeerMessageReadInit(a) => a.is_enabled(state),
        Action::PeerMessageReadError(a) => a.is_enabled(state),
        Action::PeerMessageReadSuccess(a) => a.is_enabled(state),
        Action::PeerMessageWriteNext(a) => a.is_enabled(state),
        Action::PeerMessageWriteInit(a) => a.is_enabled(state),
        Action::PeerMessageWriteError(a) => a.is_enabled(state),
        Action::PeerMessageWriteSuccess(a) => a.is_enabled(state),
        Action::PeerHandshakingInit(a) => a.is_enabled(state),
        Action::PeerHandshakingConnectionMessageInit(a) => a.is_enabled(state),
        Action::PeerHandshakingConnectionMessageEncode(a) => a.is_enabled(state),
        Action::PeerHandshakingConnectionMessageWrite(a) => a.is_enabled(state),
        Action::PeerHandshakingConnectionMessageRead(a) => a.is_enabled(state),
        Action::PeerHandshakingConnectionMessageDecode(a) => a.is_enabled(state),
        Action::PeerHandshakingEncryptionInit(a) => a.is_enabled(state),
        Action::PeerHandshakingMetadataMessageInit(a) => a.is_enabled(state),
        Action::PeerHandshakingMetadataMessageEncode(a) => a.is_enabled(state),
        Action::PeerHandshakingMetadataMessageWrite(a) => a.is_enabled(state),
        Action::PeerHandshakingMetadataMessageRead(a) => a.is_enabled(state),
        Action::PeerHandshakingMetadataMessageDecode(a) => a.is_enabled(state),
        Action::PeerHandshakingAckMessageInit(a) => a.is_enabled(state),
        Action::PeerHandshakingAckMessageEncode(a) => a.is_enabled(state),
        Action::PeerHandshakingAckMessageWrite(a) => a.is_enabled(state),
        Action::PeerHandshakingAckMessageRead(a) => a.is_enabled(state),
        Action::PeerHandshakingAckMessageDecode(a) => a.is_enabled(state),
        Action::PeerHandshakingError(a) => a.is_enabled(state),
        Action::PeerHandshakingFinish(a) => a.is_enabled(state),
        Action::Protocol(a) => a.is_enabled(state),
        Action::MempoolRecvDone(a) => a.is_enabled(state),
        Action::MempoolGetOperations(a) => a.is_enabled(state),
        Action::MempoolMarkOperationsAsPending(a) => a.is_enabled(state),
        Action::MempoolOperationRecvDone(a) => a.is_enabled(state),
        Action::MempoolOperationInject(a) => a.is_enabled(state),
        Action::MempoolValidateStart(a) => a.is_enabled(state),
        Action::MempoolValidateWaitPrevalidator(a) => a.is_enabled(state),
        Action::MempoolRpcRespond(a) => a.is_enabled(state),
        Action::MempoolRegisterOperationsStream(a) => a.is_enabled(state),
        Action::MempoolUnregisterOperationsStreams(a) => a.is_enabled(state),
        Action::MempoolSend(a) => a.is_enabled(state),
        Action::MempoolSendValidated(a) => a.is_enabled(state),
        Action::MempoolAskCurrentHead(a) => a.is_enabled(state),
        Action::MempoolBroadcast(a) => a.is_enabled(state),
        Action::MempoolBroadcastDone(a) => a.is_enabled(state),
        Action::MempoolCleanupWaitPrevalidator(a) => a.is_enabled(state),
        Action::MempoolRemoveAppliedOperations(a) => a.is_enabled(state),
        Action::MempoolGetPendingOperations(a) => a.is_enabled(state),
        Action::MempoolFlush(a) => a.is_enabled(state),
        Action::MempoolOperationDecoded(a) => a.is_enabled(state),
        Action::MempoolRpcEndorsementsStatusGet(a) => a.is_enabled(state),
        Action::PrecheckerPrecheckOperationRequest(a) => a.is_enabled(state),
        Action::PrecheckerPrecheckOperationResponse(a) => a.is_enabled(state),
        Action::PrecheckerCacheAppliedBlock(a) => a.is_enabled(state),
        Action::PrecheckerPrecheckOperationInit(a) => a.is_enabled(state),
        Action::PrecheckerGetProtocolVersion(a) => a.is_enabled(state),
        Action::PrecheckerProtocolVersionReady(a) => a.is_enabled(state),
        Action::PrecheckerDecodeOperation(a) => a.is_enabled(state),
        Action::PrecheckerOperationDecoded(a) => a.is_enabled(state),
        //Action::PrecheckerWaitForBlockApplication(a) => a.is_enabled(state),
        Action::PrecheckerWaitForBlockPrechecked(a) => a.is_enabled(state),
        Action::PrecheckerBlockPrechecked(a) => a.is_enabled(state),
        Action::PrecheckerWaitForBlockApplied(a) => a.is_enabled(state),
        Action::PrecheckerBlockApplied(a) => a.is_enabled(state),
        Action::PrecheckerGetEndorsingRights(a) => a.is_enabled(state),
        Action::PrecheckerEndorsingRightsReady(a) => a.is_enabled(state),
        Action::PrecheckerValidateEndorsement(a) => a.is_enabled(state),
        Action::PrecheckerEndorsementValidationApplied(a) => a.is_enabled(state),
        Action::PrecheckerEndorsementValidationRefused(a) => a.is_enabled(state),
        Action::PrecheckerProtocolNeeded(a) => a.is_enabled(state),
        Action::PrecheckerError(a) => a.is_enabled(state),
        Action::PrecheckerPrecacheEndorsingRights(a) => a.is_enabled(state),
        Action::PrecheckerSetNextBlockProtocol(a) => a.is_enabled(state),
        Action::PrecheckerQueryNextBlockProtocol(a) => a.is_enabled(state),
        Action::PrecheckerNextBlockProtocolReady(a) => a.is_enabled(state),
        Action::PrecheckerNextBlockProtocolError(a) => a.is_enabled(state),
        Action::PrecheckerPruneOperation(a) => a.is_enabled(state),
        Action::RightsGet(a) => a.is_enabled(state),
        Action::RightsRpcGet(a) => a.is_enabled(state),
        Action::RightsRpcEndorsingReady(a) => a.is_enabled(state),
        Action::RightsRpcBakingReady(a) => a.is_enabled(state),
        Action::RightsRpcError(a) => a.is_enabled(state),
        Action::RightsPruneRpcRequest(a) => a.is_enabled(state),
        Action::RightsInit(a) => a.is_enabled(state),
        Action::RightsGetBlockHeader(a) => a.is_enabled(state),
        Action::RightsBlockHeaderReady(a) => a.is_enabled(state),
        Action::RightsGetProtocolHash(a) => a.is_enabled(state),
        Action::RightsProtocolHashReady(a) => a.is_enabled(state),
        Action::RightsGetProtocolConstants(a) => a.is_enabled(state),
        Action::RightsProtocolConstantsReady(a) => a.is_enabled(state),
        Action::RightsGetCycleEras(a) => a.is_enabled(state),
        Action::RightsCycleErasReady(a) => a.is_enabled(state),
        Action::RightsGetCycle(a) => a.is_enabled(state),
        Action::RightsCycleReady(a) => a.is_enabled(state),
        Action::RightsGetCycleData(a) => a.is_enabled(state),
        Action::RightsCycleDataReady(a) => a.is_enabled(state),
        Action::RightsCalculateEndorsingRights(a) => a.is_enabled(state),
        Action::RightsEndorsingReady(a) => a.is_enabled(state),
        Action::RightsBakingReady(a) => a.is_enabled(state),
        Action::RightsError(a) => a.is_enabled(state),
        Action::CurrentHeadReceived(a) => a.is_enabled(state),
        Action::CurrentHeadPrecheck(a) => a.is_enabled(state),
        Action::CurrentHeadPrecheckSuccess(a) => a.is_enabled(state),
        Action::CurrentHeadPrecheckRejected(a) => a.is_enabled(state),
        Action::CurrentHeadError(a) => a.is_enabled(state),
        Action::CurrentHeadApply(a) => a.is_enabled(state),
        Action::CurrentHeadPrecacheBakingRights(a) => a.is_enabled(state),
        Action::StatsCurrentHeadReceived(a) => a.is_enabled(state),
        Action::StatsCurrentHeadPrecheckSuccess(a) => a.is_enabled(state),
        Action::StatsCurrentHeadPrepareSend(a) => a.is_enabled(state),
        Action::StatsCurrentHeadSent(a) => a.is_enabled(state),
        Action::StatsCurrentHeadSentError(a) => a.is_enabled(state),
        //Action::StatsCurrentHeadRpcGet(a) => a.is_enabled(state),
        Action::StatsCurrentHeadPrune(a) => a.is_enabled(state),
        Action::StatsCurrentHeadRpcGetPeers(a) => a.is_enabled(state),
        Action::StatsCurrentHeadRpcGetApplication(a) => a.is_enabled(state),
        Action::StorageBlockHeaderGet(a) => a.is_enabled(state),
        Action::StorageBlockHeaderOk(a) => a.is_enabled(state),
        Action::StorageBlockHeaderError(a) => a.is_enabled(state),
        Action::StorageBlockMetaGet(a) => a.is_enabled(state),
        Action::StorageBlockMetaOk(a) => a.is_enabled(state),
        Action::StorageBlockMetaError(a) => a.is_enabled(state),
        Action::StorageOperationsGet(a) => a.is_enabled(state),
        Action::StorageOperationsOk(a) => a.is_enabled(state),
        Action::StorageOperationsError(a) => a.is_enabled(state),
        Action::StorageBlockAdditionalDataGet(a) => a.is_enabled(state),
        Action::StorageBlockAdditionalDataOk(a) => a.is_enabled(state),
        Action::StorageBlockAdditionalDataError(a) => a.is_enabled(state),
        Action::StorageConstantsGet(a) => a.is_enabled(state),
        Action::StorageConstantsOk(a) => a.is_enabled(state),
        Action::StorageConstantsError(a) => a.is_enabled(state),
        Action::StorageCycleMetaGet(a) => a.is_enabled(state),
        Action::StorageCycleMetaOk(a) => a.is_enabled(state),
        Action::StorageCycleMetaError(a) => a.is_enabled(state),
        Action::StorageCycleErasGet(a) => a.is_enabled(state),
        Action::StorageCycleErasOk(a) => a.is_enabled(state),
        Action::StorageCycleErasError(a) => a.is_enabled(state),
        Action::StorageRequestCreate(a) => a.is_enabled(state),
        Action::StorageRequestInit(a) => a.is_enabled(state),
        Action::StorageRequestPending(a) => a.is_enabled(state),
        Action::StorageResponseReceived(a) => a.is_enabled(state),
        Action::StorageRequestError(a) => a.is_enabled(state),
        Action::StorageRequestSuccess(a) => a.is_enabled(state),
        Action::StorageRequestFinish(a) => a.is_enabled(state),
        Action::StorageStateSnapshotCreateInit(a) => a.is_enabled(state),
        Action::StorageStateSnapshotCreatePending(a) => a.is_enabled(state),
        Action::StorageStateSnapshotCreateError(a) => a.is_enabled(state),
        Action::StorageStateSnapshotCreateSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisCheckAppliedInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisCheckAppliedGetMetaPending(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisCheckAppliedGetMetaError(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisCheckAppliedGetMetaSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisCheckAppliedSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitHeaderPutInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitHeaderPutPending(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitHeaderPutError(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitHeaderPutSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitAdditionalDataPutInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitAdditionalDataPutPending(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitAdditionalDataPutError(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitAdditionalDataPutSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultGetInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultGetPending(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultGetError(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultGetSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultPutInit(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultPutError(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitCommitResultPutSuccess(a) => a.is_enabled(state),
        Action::StorageBlocksGenesisInitSuccess(a) => a.is_enabled(state),
        Action::ShutdownInit(a) => a.is_enabled(state),
        Action::ShutdownPending(a) => a.is_enabled(state),
        Action::ShutdownSuccess(a) => a.is_enabled(state),
        Action::ProtocolRunnerShutdownInit(a) => a.is_enabled(state),
        Action::ProtocolRunnerShutdownPending(a) => a.is_enabled(state),
        Action::ProtocolRunnerShutdownSuccess(a) => a.is_enabled(state),
    }
}

#[cfg(test)]
#[test]
fn test_all() {
    fuzzcheck::fuzz_test(action_test_all)
        .mutator(AllActionsTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
}

#[cfg(test)]
#[test]
fn test_control() {
    fuzzcheck::fuzz_test(action_test_control)
        .mutator(ControlActionTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
}

#[cfg(test)]
#[test]
fn test_dns() {
    fuzzcheck::fuzz_test(action_test_dns)
        .mutator(PeersDnsLookupActionTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
}

#[cfg(test)]
#[test]
fn test_peer() {
    fuzzcheck::fuzz_test(action_test_peer)
        .mutator(PeerActionTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
}

#[cfg(test)]
#[test]
fn test_storage() {
    fuzzcheck::fuzz_test(action_test_storage)
        .mutator(StorageActionTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
}
