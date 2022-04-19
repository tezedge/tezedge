// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use enum_dispatch::enum_dispatch;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use storage::persistent::SchemaError;

use crate::block_applier::{
    BlockApplierApplyErrorAction, BlockApplierApplyInitAction,
    BlockApplierApplyPrepareDataPendingAction, BlockApplierApplyPrepareDataSuccessAction,
    BlockApplierApplyProtocolRunnerApplyInitAction,
    BlockApplierApplyProtocolRunnerApplyPendingAction,
    BlockApplierApplyProtocolRunnerApplyRetryAction,
    BlockApplierApplyProtocolRunnerApplySuccessAction,
    BlockApplierApplyStoreApplyResultPendingAction, BlockApplierApplyStoreApplyResultSuccessAction,
    BlockApplierApplySuccessAction, BlockApplierEnqueueBlockAction,
};
use crate::current_head::*;
use crate::current_head_precheck::*;
use crate::event::{P2pPeerEvent, P2pServerEvent, WakeupEvent};
use crate::State;

use crate::paused_loops::{
    PausedLoopsAddAction, PausedLoopsResumeAllAction, PausedLoopsResumeNextInitAction,
    PausedLoopsResumeNextSuccessAction,
};

use crate::peer::binary_message::read::*;
use crate::peer::binary_message::write::*;
use crate::peer::chunk::read::*;
use crate::peer::chunk::write::*;
use crate::peer::message::read::*;
use crate::peer::message::write::*;
use crate::peer::remote_requests::block_header_get::*;
use crate::peer::remote_requests::block_operations_get::*;
use crate::peer::remote_requests::current_branch_get::*;
use crate::peer::requests::potential_peers_get::*;
use crate::peer::{
    PeerCurrentHeadUpdateAction, PeerTryReadLoopFinishAction, PeerTryReadLoopStartAction,
    PeerTryWriteLoopFinishAction, PeerTryWriteLoopStartAction,
};

use crate::peer::connection::closed::PeerConnectionClosedAction;
use crate::peer::connection::incoming::accept::*;
use crate::peer::connection::incoming::{
    PeerConnectionIncomingErrorAction, PeerConnectionIncomingSuccessAction,
};
use crate::peer::connection::outgoing::{
    PeerConnectionOutgoingErrorAction, PeerConnectionOutgoingInitAction,
    PeerConnectionOutgoingPendingAction, PeerConnectionOutgoingRandomInitAction,
    PeerConnectionOutgoingSuccessAction,
};
use crate::peer::disconnection::{PeerDisconnectAction, PeerDisconnectedAction};

use crate::peer::handshaking::*;

use crate::mempool::validator::*;
use crate::peers::add::multi::PeersAddMultiAction;
use crate::peers::add::PeersAddIncomingPeerAction;
use crate::peers::check::timeouts::{
    PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction, PeersCheckTimeoutsSuccessAction,
};
use crate::peers::dns_lookup::{
    PeersDnsLookupCleanupAction, PeersDnsLookupErrorAction, PeersDnsLookupInitAction,
    PeersDnsLookupSuccessAction,
};
use crate::peers::graylist::{
    PeersGraylistAddressAction, PeersGraylistIpAddAction, PeersGraylistIpAddedAction,
    PeersGraylistIpRemoveAction, PeersGraylistIpRemovedAction,
};
use crate::peers::init::PeersInitAction;
use crate::peers::remove::PeersRemoveAction;
use crate::prechecker::prechecker_actions::*;

use crate::rights::rights_actions::*;

use crate::bootstrap::*;
use crate::mempool::mempool_actions::*;

use crate::protocol_runner::current_head::{
    ProtocolRunnerCurrentHeadErrorAction, ProtocolRunnerCurrentHeadInitAction,
    ProtocolRunnerCurrentHeadPendingAction, ProtocolRunnerCurrentHeadSuccessAction,
};
use crate::protocol_runner::init::context::{
    ProtocolRunnerInitContextAction, ProtocolRunnerInitContextErrorAction,
    ProtocolRunnerInitContextPendingAction, ProtocolRunnerInitContextSuccessAction,
};
use crate::protocol_runner::init::context_ipc_server::{
    ProtocolRunnerInitContextIpcServerAction, ProtocolRunnerInitContextIpcServerErrorAction,
    ProtocolRunnerInitContextIpcServerPendingAction,
    ProtocolRunnerInitContextIpcServerSuccessAction,
};
use crate::protocol_runner::init::runtime::{
    ProtocolRunnerInitRuntimeAction, ProtocolRunnerInitRuntimeErrorAction,
    ProtocolRunnerInitRuntimePendingAction, ProtocolRunnerInitRuntimeSuccessAction,
};
use crate::protocol_runner::init::{
    ProtocolRunnerInitAction, ProtocolRunnerInitCheckGenesisAppliedAction,
    ProtocolRunnerInitCheckGenesisAppliedSuccessAction, ProtocolRunnerInitSuccessAction,
};
use crate::protocol_runner::spawn_server::{
    ProtocolRunnerSpawnServerErrorAction, ProtocolRunnerSpawnServerInitAction,
    ProtocolRunnerSpawnServerPendingAction, ProtocolRunnerSpawnServerSuccessAction,
};
use crate::protocol_runner::{
    ProtocolRunnerNotifyStatusAction, ProtocolRunnerReadyAction, ProtocolRunnerResponseAction,
    ProtocolRunnerResponseUnexpectedAction, ProtocolRunnerShutdownInitAction,
    ProtocolRunnerShutdownPendingAction, ProtocolRunnerShutdownSuccessAction,
    ProtocolRunnerStartAction,
};

use crate::rpc::rpc_actions::*;
use crate::stats::current_head::stats_current_head_actions::*;
use crate::storage::blocks::genesis::check_applied::{
    StorageBlocksGenesisCheckAppliedGetMetaErrorAction,
    StorageBlocksGenesisCheckAppliedGetMetaPendingAction,
    StorageBlocksGenesisCheckAppliedGetMetaSuccessAction,
    StorageBlocksGenesisCheckAppliedInitAction, StorageBlocksGenesisCheckAppliedSuccessAction,
};
use crate::storage::blocks::genesis::init::additional_data_put::{
    StorageBlocksGenesisInitAdditionalDataPutErrorAction,
    StorageBlocksGenesisInitAdditionalDataPutInitAction,
    StorageBlocksGenesisInitAdditionalDataPutPendingAction,
    StorageBlocksGenesisInitAdditionalDataPutSuccessAction,
};
use crate::storage::blocks::genesis::init::commit_result_get::{
    StorageBlocksGenesisInitCommitResultGetErrorAction,
    StorageBlocksGenesisInitCommitResultGetInitAction,
    StorageBlocksGenesisInitCommitResultGetPendingAction,
    StorageBlocksGenesisInitCommitResultGetSuccessAction,
};
use crate::storage::blocks::genesis::init::commit_result_put::{
    StorageBlocksGenesisInitCommitResultPutErrorAction,
    StorageBlocksGenesisInitCommitResultPutInitAction,
    StorageBlocksGenesisInitCommitResultPutSuccessAction,
};
use crate::storage::blocks::genesis::init::header_put::{
    StorageBlocksGenesisInitHeaderPutErrorAction, StorageBlocksGenesisInitHeaderPutInitAction,
    StorageBlocksGenesisInitHeaderPutPendingAction, StorageBlocksGenesisInitHeaderPutSuccessAction,
};
use crate::storage::blocks::genesis::init::{
    StorageBlocksGenesisInitAction, StorageBlocksGenesisInitSuccessAction,
};
use crate::storage::request::{
    StorageRequestCreateAction, StorageRequestErrorAction, StorageRequestFinishAction,
    StorageRequestInitAction, StorageRequestPendingAction, StorageRequestSuccessAction,
    StorageResponseReceivedAction,
};
use crate::storage::state_snapshot::create::{
    StorageStateSnapshotCreateErrorAction, StorageStateSnapshotCreateInitAction,
    StorageStateSnapshotCreatePendingAction, StorageStateSnapshotCreateSuccessAction,
};
use crate::storage::{
    kv_block_additional_data, kv_block_header, kv_block_meta, kv_constants, kv_cycle_eras,
    kv_cycle_meta, kv_operations,
};

use crate::shutdown::{ShutdownInitAction, ShutdownPendingAction, ShutdownSuccessAction};

pub use redux_rs::{ActionId, EnablingCondition};

pub type ActionWithMeta = redux_rs::ActionWithMeta<Action>;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InitAction {}

impl EnablingCondition<State> for InitAction {
    fn is_enabled(&self, _: &State) -> bool {
        false
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MioWaitForEventsAction {}

impl EnablingCondition<State> for MioWaitForEventsAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MioTimeoutEvent {}

impl EnablingCondition<State> for MioTimeoutEvent {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[enum_dispatch]
trait EnablingConditionDispatched {
    fn is_enabled(&self, state: &State) -> bool;
}

impl<T> EnablingConditionDispatched for T
where
    T: EnablingCondition<State>,
{
    fn is_enabled(&self, state: &State) -> bool {
        EnablingCondition::is_enabled(self, state)
    }
}

#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(
    ActionKind,
    derive(
        strum_macros::EnumIter,
        strum_macros::Display,
        Serialize,
        Deserialize,
        Hash,
        Ord,
        PartialOrd,
    )
)]
#[serde(tag = "kind", content = "content")]
#[enum_dispatch(EnablingConditionDispatched)]
pub enum Action {
    Init(InitAction),

    PausedLoopsAdd(PausedLoopsAddAction),
    PausedLoopsResumeAll(PausedLoopsResumeAllAction),
    PausedLoopsResumeNextInit(PausedLoopsResumeNextInitAction),
    PausedLoopsResumeNextSuccess(PausedLoopsResumeNextSuccessAction),

    ProtocolRunnerStart(ProtocolRunnerStartAction),

    ProtocolRunnerSpawnServerInit(ProtocolRunnerSpawnServerInitAction),
    ProtocolRunnerSpawnServerPending(ProtocolRunnerSpawnServerPendingAction),
    ProtocolRunnerSpawnServerError(ProtocolRunnerSpawnServerErrorAction),
    ProtocolRunnerSpawnServerSuccess(ProtocolRunnerSpawnServerSuccessAction),

    ProtocolRunnerCurrentHeadInit(ProtocolRunnerCurrentHeadInitAction),
    ProtocolRunnerCurrentHeadPending(ProtocolRunnerCurrentHeadPendingAction),
    ProtocolRunnerCurrentHeadError(ProtocolRunnerCurrentHeadErrorAction),
    ProtocolRunnerCurrentHeadSuccess(ProtocolRunnerCurrentHeadSuccessAction),

    ProtocolRunnerInit(ProtocolRunnerInitAction),

    ProtocolRunnerInitRuntime(ProtocolRunnerInitRuntimeAction),
    ProtocolRunnerInitRuntimePending(ProtocolRunnerInitRuntimePendingAction),
    ProtocolRunnerInitRuntimeError(ProtocolRunnerInitRuntimeErrorAction),
    ProtocolRunnerInitRuntimeSuccess(ProtocolRunnerInitRuntimeSuccessAction),

    ProtocolRunnerInitCheckGenesisApplied(ProtocolRunnerInitCheckGenesisAppliedAction),
    ProtocolRunnerInitCheckGenesisAppliedSuccess(
        ProtocolRunnerInitCheckGenesisAppliedSuccessAction,
    ),

    ProtocolRunnerInitContext(ProtocolRunnerInitContextAction),
    ProtocolRunnerInitContextPending(ProtocolRunnerInitContextPendingAction),
    ProtocolRunnerInitContextError(ProtocolRunnerInitContextErrorAction),
    ProtocolRunnerInitContextSuccess(ProtocolRunnerInitContextSuccessAction),

    ProtocolRunnerInitContextIpcServer(ProtocolRunnerInitContextIpcServerAction),
    ProtocolRunnerInitContextIpcServerPending(ProtocolRunnerInitContextIpcServerPendingAction),
    ProtocolRunnerInitContextIpcServerError(ProtocolRunnerInitContextIpcServerErrorAction),
    ProtocolRunnerInitContextIpcServerSuccess(ProtocolRunnerInitContextIpcServerSuccessAction),

    ProtocolRunnerInitSuccess(ProtocolRunnerInitSuccessAction),

    ProtocolRunnerReady(ProtocolRunnerReadyAction),
    ProtocolRunnerNotifyStatus(ProtocolRunnerNotifyStatusAction),

    ProtocolRunnerResponse(ProtocolRunnerResponseAction),
    ProtocolRunnerResponseUnexpected(ProtocolRunnerResponseUnexpectedAction),

    CurrentHeadRehydrateInit(CurrentHeadRehydrateInitAction),
    CurrentHeadRehydratePending(CurrentHeadRehydratePendingAction),
    CurrentHeadRehydrateError(CurrentHeadRehydrateErrorAction),
    CurrentHeadRehydrateSuccess(CurrentHeadRehydrateSuccessAction),
    CurrentHeadRehydrated(CurrentHeadRehydratedAction),
    CurrentHeadUpdate(CurrentHeadUpdateAction),

    BlockApplierEnqueueBlock(BlockApplierEnqueueBlockAction),

    BlockApplierApplyInit(BlockApplierApplyInitAction),

    BlockApplierApplyPrepareDataPending(BlockApplierApplyPrepareDataPendingAction),
    BlockApplierApplyPrepareDataSuccess(BlockApplierApplyPrepareDataSuccessAction),

    BlockApplierApplyProtocolRunnerApplyInit(BlockApplierApplyProtocolRunnerApplyInitAction),
    BlockApplierApplyProtocolRunnerApplyPending(BlockApplierApplyProtocolRunnerApplyPendingAction),
    BlockApplierApplyProtocolRunnerApplyRetry(BlockApplierApplyProtocolRunnerApplyRetryAction),
    BlockApplierApplyProtocolRunnerApplySuccess(BlockApplierApplyProtocolRunnerApplySuccessAction),

    BlockApplierApplyStoreApplyResultPending(BlockApplierApplyStoreApplyResultPendingAction),
    BlockApplierApplyStoreApplyResultSuccess(BlockApplierApplyStoreApplyResultSuccessAction),

    BlockApplierApplyError(BlockApplierApplyErrorAction),
    BlockApplierApplySuccess(BlockApplierApplySuccessAction),

    PeersInit(PeersInitAction),

    PeersDnsLookupInit(PeersDnsLookupInitAction),
    PeersDnsLookupError(PeersDnsLookupErrorAction),
    PeersDnsLookupSuccess(PeersDnsLookupSuccessAction),
    PeersDnsLookupCleanup(PeersDnsLookupCleanupAction),

    PeersGraylistAddress(PeersGraylistAddressAction),
    PeersGraylistIpAdd(PeersGraylistIpAddAction),
    PeersGraylistIpAdded(PeersGraylistIpAddedAction),
    PeersGraylistIpRemove(PeersGraylistIpRemoveAction),
    PeersGraylistIpRemoved(PeersGraylistIpRemovedAction),

    PeersAddIncomingPeer(PeersAddIncomingPeerAction),
    PeersAddMulti(PeersAddMultiAction),
    PeersRemove(PeersRemoveAction),

    PeersCheckTimeoutsInit(PeersCheckTimeoutsInitAction),
    PeersCheckTimeoutsSuccess(PeersCheckTimeoutsSuccessAction),
    PeersCheckTimeoutsCleanup(PeersCheckTimeoutsCleanupAction),

    PeerConnectionIncomingAccept(PeerConnectionIncomingAcceptAction),
    PeerConnectionIncomingAcceptError(PeerConnectionIncomingAcceptErrorAction),
    PeerConnectionIncomingRejected(PeerConnectionIncomingRejectedAction),
    PeerConnectionIncomingAcceptSuccess(PeerConnectionIncomingAcceptSuccessAction),

    PeerConnectionIncomingError(PeerConnectionIncomingErrorAction),
    PeerConnectionIncomingSuccess(PeerConnectionIncomingSuccessAction),

    PeerConnectionOutgoingRandomInit(PeerConnectionOutgoingRandomInitAction),
    PeerConnectionOutgoingInit(PeerConnectionOutgoingInitAction),
    PeerConnectionOutgoingPending(PeerConnectionOutgoingPendingAction),
    PeerConnectionOutgoingError(PeerConnectionOutgoingErrorAction),
    PeerConnectionOutgoingSuccess(PeerConnectionOutgoingSuccessAction),

    PeerConnectionClosed(PeerConnectionClosedAction),

    PeerDisconnect(PeerDisconnectAction),
    PeerDisconnected(PeerDisconnectedAction),

    MioWaitForEvents(MioWaitForEventsAction),
    MioTimeoutEvent(MioTimeoutEvent),
    P2pServerEvent(P2pServerEvent),
    P2pPeerEvent(P2pPeerEvent),
    WakeupEvent(WakeupEvent),

    PeerTryWriteLoopStart(PeerTryWriteLoopStartAction),
    PeerTryWriteLoopFinish(PeerTryWriteLoopFinishAction),
    PeerTryReadLoopStart(PeerTryReadLoopStartAction),
    PeerTryReadLoopFinish(PeerTryReadLoopFinishAction),

    // chunk read
    PeerChunkReadInit(PeerChunkReadInitAction),
    PeerChunkReadPart(PeerChunkReadPartAction),
    PeerChunkReadDecrypt(PeerChunkReadDecryptAction),
    PeerChunkReadReady(PeerChunkReadReadyAction),
    PeerChunkReadError(PeerChunkReadErrorAction),

    // chunk write
    PeerChunkWriteSetContent(PeerChunkWriteSetContentAction),
    PeerChunkWriteEncryptContent(PeerChunkWriteEncryptContentAction),
    PeerChunkWriteCreateChunk(PeerChunkWriteCreateChunkAction),
    PeerChunkWritePart(PeerChunkWritePartAction),
    PeerChunkWriteReady(PeerChunkWriteReadyAction),
    PeerChunkWriteError(PeerChunkWriteErrorAction),

    // binary message read
    PeerBinaryMessageReadInit(PeerBinaryMessageReadInitAction),
    PeerBinaryMessageReadChunkReady(PeerBinaryMessageReadChunkReadyAction),
    PeerBinaryMessageReadSizeReady(PeerBinaryMessageReadSizeReadyAction),
    PeerBinaryMessageReadReady(PeerBinaryMessageReadReadyAction),
    PeerBinaryMessageReadError(PeerBinaryMessageReadErrorAction),

    // binary message write
    PeerBinaryMessageWriteSetContent(PeerBinaryMessageWriteSetContentAction),
    PeerBinaryMessageWriteNextChunk(PeerBinaryMessageWriteNextChunkAction),
    PeerBinaryMessageWriteReady(PeerBinaryMessageWriteReadyAction),
    PeerBinaryMessageWriteError(PeerBinaryMessageWriteErrorAction),

    PeerMessageReadInit(PeerMessageReadInitAction),
    PeerMessageReadError(PeerMessageReadErrorAction),
    PeerMessageReadSuccess(PeerMessageReadSuccessAction),

    PeerMessageWriteNext(PeerMessageWriteNextAction),
    PeerMessageWriteInit(PeerMessageWriteInitAction),
    PeerMessageWriteError(PeerMessageWriteErrorAction),
    PeerMessageWriteSuccess(PeerMessageWriteSuccessAction),

    PeerHandshakingInit(PeerHandshakingInitAction),
    PeerHandshakingConnectionMessageInit(PeerHandshakingConnectionMessageInitAction),
    PeerHandshakingConnectionMessageEncode(PeerHandshakingConnectionMessageEncodeAction),
    PeerHandshakingConnectionMessageWrite(PeerHandshakingConnectionMessageWriteAction),
    PeerHandshakingConnectionMessageRead(PeerHandshakingConnectionMessageReadAction),
    PeerHandshakingConnectionMessageDecode(PeerHandshakingConnectionMessageDecodeAction),

    PeerHandshakingEncryptionInit(PeerHandshakingEncryptionInitAction),

    PeerHandshakingMetadataMessageInit(PeerHandshakingMetadataMessageInitAction),
    PeerHandshakingMetadataMessageEncode(PeerHandshakingMetadataMessageEncodeAction),
    PeerHandshakingMetadataMessageWrite(PeerHandshakingMetadataMessageWriteAction),
    PeerHandshakingMetadataMessageRead(PeerHandshakingMetadataMessageReadAction),
    PeerHandshakingMetadataMessageDecode(PeerHandshakingMetadataMessageDecodeAction),

    PeerHandshakingAckMessageInit(PeerHandshakingAckMessageInitAction),
    PeerHandshakingAckMessageEncode(PeerHandshakingAckMessageEncodeAction),
    PeerHandshakingAckMessageWrite(PeerHandshakingAckMessageWriteAction),
    PeerHandshakingAckMessageRead(PeerHandshakingAckMessageReadAction),
    PeerHandshakingAckMessageDecode(PeerHandshakingAckMessageDecodeAction),

    PeerHandshakingError(PeerHandshakingErrorAction),
    PeerHandshakingFinish(PeerHandshakingFinishAction),

    PeerCurrentHeadUpdate(PeerCurrentHeadUpdateAction),

    PeerRequestsPotentialPeersGetInit(PeerRequestsPotentialPeersGetInitAction),
    PeerRequestsPotentialPeersGetPending(PeerRequestsPotentialPeersGetPendingAction),
    PeerRequestsPotentialPeersGetError(PeerRequestsPotentialPeersGetErrorAction),
    PeerRequestsPotentialPeersGetSuccess(PeerRequestsPotentialPeersGetSuccessAction),
    PeerRequestsPotentialPeersGetFinish(PeerRequestsPotentialPeersGetFinishAction),

    PeerRemoteRequestsBlockHeaderGetEnqueue(PeerRemoteRequestsBlockHeaderGetEnqueueAction),
    PeerRemoteRequestsBlockHeaderGetInitNext(PeerRemoteRequestsBlockHeaderGetInitNextAction),
    PeerRemoteRequestsBlockHeaderGetPending(PeerRemoteRequestsBlockHeaderGetPendingAction),
    PeerRemoteRequestsBlockHeaderGetError(PeerRemoteRequestsBlockHeaderGetErrorAction),
    PeerRemoteRequestsBlockHeaderGetSuccess(PeerRemoteRequestsBlockHeaderGetSuccessAction),
    PeerRemoteRequestsBlockHeaderGetFinish(PeerRemoteRequestsBlockHeaderGetFinishAction),

    PeerRemoteRequestsBlockOperationsGetEnqueue(PeerRemoteRequestsBlockOperationsGetEnqueueAction),
    PeerRemoteRequestsBlockOperationsGetInitNext(
        PeerRemoteRequestsBlockOperationsGetInitNextAction,
    ),
    PeerRemoteRequestsBlockOperationsGetPending(PeerRemoteRequestsBlockOperationsGetPendingAction),
    PeerRemoteRequestsBlockOperationsGetError(PeerRemoteRequestsBlockOperationsGetErrorAction),
    PeerRemoteRequestsBlockOperationsGetSuccess(PeerRemoteRequestsBlockOperationsGetSuccessAction),
    PeerRemoteRequestsBlockOperationsGetFinish(PeerRemoteRequestsBlockOperationsGetFinishAction),

    PeerRemoteRequestsCurrentBranchGetInit(PeerRemoteRequestsCurrentBranchGetInitAction),
    PeerRemoteRequestsCurrentBranchGetPending(PeerRemoteRequestsCurrentBranchGetPendingAction),
    PeerRemoteRequestsCurrentBranchGetNextBlockInit(
        PeerRemoteRequestsCurrentBranchGetNextBlockInitAction,
    ),
    PeerRemoteRequestsCurrentBranchGetNextBlockPending(
        PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction,
    ),
    PeerRemoteRequestsCurrentBranchGetNextBlockError(
        PeerRemoteRequestsCurrentBranchGetNextBlockErrorAction,
    ),
    PeerRemoteRequestsCurrentBranchGetNextBlockSuccess(
        PeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction,
    ),
    PeerRemoteRequestsCurrentBranchGetSuccess(PeerRemoteRequestsCurrentBranchGetSuccessAction),
    PeerRemoteRequestsCurrentBranchGetFinish(PeerRemoteRequestsCurrentBranchGetFinishAction),

    BootstrapInit(BootstrapInitAction),

    BootstrapPeersConnectPending(BootstrapPeersConnectPendingAction),
    BootstrapPeersConnectSuccess(BootstrapPeersConnectSuccessAction),

    BootstrapPeersMainBranchFindInit(BootstrapPeersMainBranchFindInitAction),
    BootstrapPeersMainBranchFindPending(BootstrapPeersMainBranchFindPendingAction),
    BootstrapPeerCurrentBranchReceived(BootstrapPeerCurrentBranchReceivedAction),
    BootstrapPeersMainBranchFindSuccess(BootstrapPeersMainBranchFindSuccessAction),

    BootstrapPeersBlockHeadersGetInit(BootstrapPeersBlockHeadersGetInitAction),
    BootstrapPeersBlockHeadersGetPending(BootstrapPeersBlockHeadersGetPendingAction),

    BootstrapPeerBlockHeaderGetInit(BootstrapPeerBlockHeaderGetInitAction),
    BootstrapPeerBlockHeaderGetPending(BootstrapPeerBlockHeaderGetPendingAction),
    BootstrapPeerBlockHeaderGetTimeout(BootstrapPeerBlockHeaderGetTimeoutAction),
    BootstrapPeerBlockHeaderGetSuccess(BootstrapPeerBlockHeaderGetSuccessAction),
    BootstrapPeerBlockHeaderGetFinish(BootstrapPeerBlockHeaderGetFinishAction),

    BootstrapPeersBlockHeadersGetSuccess(BootstrapPeersBlockHeadersGetSuccessAction),

    BootstrapPeersBlockOperationsGetInit(BootstrapPeersBlockOperationsGetInitAction),
    BootstrapPeersBlockOperationsGetPending(BootstrapPeersBlockOperationsGetPendingAction),
    BootstrapPeersBlockOperationsGetNextAll(BootstrapPeersBlockOperationsGetNextAllAction),
    BootstrapPeersBlockOperationsGetNext(BootstrapPeersBlockOperationsGetNextAction),
    BootstrapPeerBlockOperationsGetPending(BootstrapPeerBlockOperationsGetPendingAction),
    BootstrapPeerBlockOperationsGetTimeout(BootstrapPeerBlockOperationsGetTimeoutAction),
    BootstrapPeerBlockOperationsGetRetry(BootstrapPeerBlockOperationsGetRetryAction),
    BootstrapPeerBlockOperationsReceived(BootstrapPeerBlockOperationsReceivedAction),
    BootstrapPeerBlockOperationsGetSuccess(BootstrapPeerBlockOperationsGetSuccessAction),

    BootstrapScheduleBlocksForApply(BootstrapScheduleBlocksForApplyAction),
    BootstrapScheduleBlockForApply(BootstrapScheduleBlockForApplyAction),

    BootstrapPeersBlockOperationsGetSuccess(BootstrapPeersBlockOperationsGetSuccessAction),

    BootstrapCheckTimeoutsInit(BootstrapCheckTimeoutsInitAction),

    BootstrapError(BootstrapErrorAction),
    BootstrapFinished(BootstrapFinishedAction),
    BootstrapFromPeerCurrentHead(BootstrapFromPeerCurrentHeadAction),

    MempoolRecvDone(MempoolRecvDoneAction),
    MempoolGetOperations(MempoolGetOperationsAction),
    MempoolMarkOperationsAsPending(MempoolMarkOperationsAsPendingAction),
    MempoolOperationRecvDone(MempoolOperationRecvDoneAction),
    MempoolOperationInject(MempoolOperationInjectAction),
    MempoolRpcRespond(MempoolRpcRespondAction),
    MempoolRegisterOperationsStream(MempoolRegisterOperationsStreamAction),
    MempoolUnregisterOperationsStreams(MempoolUnregisterOperationsStreamsAction),
    MempoolSend(MempoolSendAction),
    MempoolSendValidated(MempoolSendValidatedAction),
    MempoolAskCurrentHead(MempoolAskCurrentHeadAction),
    MempoolBroadcast(MempoolBroadcastAction),
    MempoolBroadcastDone(MempoolBroadcastDoneAction),
    MempoolGetPendingOperations(MempoolGetPendingOperationsAction),
    MempoolOperationDecoded(MempoolOperationDecodedAction),
    MempoolRpcEndorsementsStatusGet(MempoolRpcEndorsementsStatusGetAction),
    MempoolOperationValidateNext(MempoolOperationValidateNextAction),

    MempoolValidatorInit(MempoolValidatorInitAction),
    MempoolValidatorPending(MempoolValidatorPendingAction),
    MempoolValidatorSuccess(MempoolValidatorSuccessAction),
    MempoolValidatorReady(MempoolValidatorReadyAction),

    MempoolValidatorValidateInit(MempoolValidatorValidateInitAction),
    MempoolValidatorValidatePending(MempoolValidatorValidatePendingAction),
    MempoolValidatorValidateSuccess(MempoolValidatorValidateSuccessAction),

    BlockInject(BlockInjectAction),

    PrecheckerPrecheckOperationRequest(PrecheckerPrecheckOperationRequestAction),
    PrecheckerPrecheckOperationResponse(PrecheckerPrecheckOperationResponseAction),
    PrecheckerPrecheckBlock(PrecheckerPrecheckBlockAction),
    PrecheckerCacheAppliedBlock(PrecheckerCacheAppliedBlockAction),
    PrecheckerPrecheckOperationInit(PrecheckerPrecheckOperationInitAction),
    PrecheckerDecodeOperation(PrecheckerDecodeOperationAction),
    PrecheckerOperationDecoded(PrecheckerOperationDecodedAction),
    PrecheckerWaitForBlockPrechecked(PrecheckerWaitForBlockPrecheckedAction),
    PrecheckerBlockPrechecked(PrecheckerBlockPrecheckedAction),
    PrecheckerWaitForBlockApplied(PrecheckerWaitForBlockAppliedAction),
    PrecheckerBlockApplied(PrecheckerBlockAppliedAction),
    PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction),
    PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction),
    PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction),
    PrecheckerEndorsementValidationApplied(PrecheckerEndorsementValidationAppliedAction),
    PrecheckerEndorsementValidationRefused(PrecheckerEndorsementValidationRefusedAction),
    PrecheckerProtocolNeeded(PrecheckerProtocolNeededAction),
    PrecheckerError(PrecheckerErrorAction),
    PrecheckerPrecacheEndorsingRights(PrecheckerPrecacheEndorsingRightsAction),
    PrecheckerPruneOperation(PrecheckerPruneOperationAction),
    PrecheckerCacheProtocol(PrecheckerCacheProtocolAction),

    RightsGet(RightsGetAction),
    RightsRpcGet(RightsRpcGetAction),
    RightsRpcEndorsingReady(RightsRpcEndorsingReadyAction),
    RightsRpcBakingReady(RightsRpcBakingReadyAction),
    RightsRpcError(RightsRpcErrorAction),
    RightsPruneRpcRequest(RightsRpcPruneAction),
    RightsInit(RightsInitAction),
    RightsGetBlockHeader(RightsGetBlockHeaderAction),
    RightsBlockHeaderReady(RightsBlockHeaderReadyAction),
    RightsGetProtocolHash(RightsGetProtocolHashAction),
    RightsProtocolHashReady(RightsProtocolHashReadyAction),
    RightsGetProtocolConstants(RightsGetProtocolConstantsAction),
    RightsProtocolConstantsReady(RightsProtocolConstantsReadyAction),
    RightsGetCycleEras(RightsGetCycleErasAction),
    RightsCycleErasReady(RightsCycleErasReadyAction),
    RightsGetCycle(RightsGetCycleAction),
    RightsCycleReady(RightsCycleReadyAction),
    RightsGetCycleData(RightsGetCycleDataAction),
    RightsCycleDataReady(RightsCycleDataReadyAction),
    RightsCalculateEndorsingRights(RightsCalculateAction),
    RightsEndorsingReady(RightsEndorsingReadyAction),
    RightsBakingReady(RightsBakingReadyAction),
    RightsError(RightsErrorAction),

    CurrentHeadReceived(CurrentHeadReceivedAction),
    CurrentHeadPrecheck(CurrentHeadPrecheckAction),
    CurrentHeadPrecheckSuccess(CurrentHeadPrecheckSuccessAction),
    CurrentHeadPrecheckRejected(CurrentHeadPrecheckRejectedAction),
    CurrentHeadError(CurrentHeadErrorAction),
    CurrentHeadPrecacheBakingRights(CurrentHeadPrecacheBakingRightsAction),

    StatsCurrentHeadPrecheckInit(StatsCurrentHeadPrecheckInitAction),
    StatsCurrentHeadPrecheckSuccess(StatsCurrentHeadPrecheckSuccessAction),
    StatsCurrentHeadPrepareSend(StatsCurrentHeadPrepareSendAction),
    StatsCurrentHeadSent(StatsCurrentHeadSentAction),
    StatsCurrentHeadSentError(StatsCurrentHeadSentErrorAction),

    RpcBootstrapped(RpcBootstrappedAction),
    RpcBootstrappedNewBlock(RpcBootstrappedNewBlockAction),
    RpcBootstrappedDone(RpcBootstrappedDoneAction),
    RpcMonitorValidBlocks(RpcMonitorValidBlocksAction),
    RpcReplyValidBlock(RpcReplyValidBlockAction),
    RpcInjectBlock(RpcInjectBlockAction),
    RpcRejectOutdatedInjectedBlock(RpcRejectOutdatedInjectedBlockAction),

    StorageBlockHeaderGet(kv_block_header::StorageBlockHeaderGetAction),
    StorageBlockHeaderOk(kv_block_header::StorageBlockHeaderOkAction),
    StorageBlockHeaderError(kv_block_header::StorageBlockHeaderErrorAction),

    StorageBlockMetaGet(kv_block_meta::StorageBlockMetaGetAction),
    StorageBlockMetaOk(kv_block_meta::StorageBlockMetaOkAction),
    StorageBlockMetaError(kv_block_meta::StorageBlockMetaErrorAction),

    StorageOperationsGet(kv_operations::StorageOperationsGetAction),
    StorageOperationsOk(kv_operations::StorageOperationsOkAction),
    StorageOperationsError(kv_operations::StorageOperationsErrorAction),

    StorageBlockAdditionalDataGet(kv_block_additional_data::StorageBlockAdditionalDataGetAction),
    StorageBlockAdditionalDataOk(kv_block_additional_data::StorageBlockAdditionalDataOkAction),
    StorageBlockAdditionalDataError(
        kv_block_additional_data::StorageBlockAdditionalDataErrorAction,
    ),

    StorageConstantsGet(kv_constants::StorageConstantsGetAction),
    StorageConstantsOk(kv_constants::StorageConstantsOkAction),
    StorageConstantsError(kv_constants::StorageConstantsErrorAction),

    StorageCycleMetaGet(kv_cycle_meta::StorageCycleMetaGetAction),
    StorageCycleMetaOk(kv_cycle_meta::StorageCycleMetaOkAction),
    StorageCycleMetaError(kv_cycle_meta::StorageCycleMetaErrorAction),

    StorageCycleErasGet(kv_cycle_eras::StorageCycleErasGetAction),
    StorageCycleErasOk(kv_cycle_eras::StorageCycleErasOkAction),
    StorageCycleErasError(kv_cycle_eras::StorageCycleErasErrorAction),

    StorageRequestCreate(StorageRequestCreateAction),
    StorageRequestInit(StorageRequestInitAction),
    StorageRequestPending(StorageRequestPendingAction),
    StorageResponseReceived(StorageResponseReceivedAction),
    StorageRequestError(StorageRequestErrorAction),
    StorageRequestSuccess(StorageRequestSuccessAction),
    StorageRequestFinish(StorageRequestFinishAction),

    StorageStateSnapshotCreateInit(StorageStateSnapshotCreateInitAction),
    StorageStateSnapshotCreatePending(StorageStateSnapshotCreatePendingAction),
    StorageStateSnapshotCreateError(StorageStateSnapshotCreateErrorAction),
    StorageStateSnapshotCreateSuccess(StorageStateSnapshotCreateSuccessAction),

    StorageBlocksGenesisCheckAppliedInit(StorageBlocksGenesisCheckAppliedInitAction),
    StorageBlocksGenesisCheckAppliedGetMetaPending(
        StorageBlocksGenesisCheckAppliedGetMetaPendingAction,
    ),
    StorageBlocksGenesisCheckAppliedGetMetaError(
        StorageBlocksGenesisCheckAppliedGetMetaErrorAction,
    ),
    StorageBlocksGenesisCheckAppliedGetMetaSuccess(
        StorageBlocksGenesisCheckAppliedGetMetaSuccessAction,
    ),
    StorageBlocksGenesisCheckAppliedSuccess(StorageBlocksGenesisCheckAppliedSuccessAction),

    StorageBlocksGenesisInit(StorageBlocksGenesisInitAction),

    StorageBlocksGenesisInitHeaderPutInit(StorageBlocksGenesisInitHeaderPutInitAction),
    StorageBlocksGenesisInitHeaderPutPending(StorageBlocksGenesisInitHeaderPutPendingAction),
    StorageBlocksGenesisInitHeaderPutError(StorageBlocksGenesisInitHeaderPutErrorAction),
    StorageBlocksGenesisInitHeaderPutSuccess(StorageBlocksGenesisInitHeaderPutSuccessAction),

    StorageBlocksGenesisInitAdditionalDataPutInit(
        StorageBlocksGenesisInitAdditionalDataPutInitAction,
    ),
    StorageBlocksGenesisInitAdditionalDataPutPending(
        StorageBlocksGenesisInitAdditionalDataPutPendingAction,
    ),
    StorageBlocksGenesisInitAdditionalDataPutError(
        StorageBlocksGenesisInitAdditionalDataPutErrorAction,
    ),
    StorageBlocksGenesisInitAdditionalDataPutSuccess(
        StorageBlocksGenesisInitAdditionalDataPutSuccessAction,
    ),

    StorageBlocksGenesisInitCommitResultGetInit(StorageBlocksGenesisInitCommitResultGetInitAction),
    StorageBlocksGenesisInitCommitResultGetPending(
        StorageBlocksGenesisInitCommitResultGetPendingAction,
    ),
    StorageBlocksGenesisInitCommitResultGetError(
        StorageBlocksGenesisInitCommitResultGetErrorAction,
    ),
    StorageBlocksGenesisInitCommitResultGetSuccess(
        StorageBlocksGenesisInitCommitResultGetSuccessAction,
    ),

    StorageBlocksGenesisInitCommitResultPutInit(StorageBlocksGenesisInitCommitResultPutInitAction),
    StorageBlocksGenesisInitCommitResultPutError(
        StorageBlocksGenesisInitCommitResultPutErrorAction,
    ),
    StorageBlocksGenesisInitCommitResultPutSuccess(
        StorageBlocksGenesisInitCommitResultPutSuccessAction,
    ),

    StorageBlocksGenesisInitSuccess(StorageBlocksGenesisInitSuccessAction),

    ShutdownInit(ShutdownInitAction),
    ShutdownPending(ShutdownPendingAction),
    ShutdownSuccess(ShutdownSuccessAction),

    ProtocolRunnerShutdownInit(ProtocolRunnerShutdownInitAction),
    ProtocolRunnerShutdownPending(ProtocolRunnerShutdownPendingAction),
    ProtocolRunnerShutdownSuccess(ProtocolRunnerShutdownSuccessAction),
}

impl Action {
    #[inline(always)]
    pub fn kind(&self) -> ActionKind {
        ActionKind::from(self)
    }

    #[inline(always)]
    pub fn is_enabled(&self, state: &State) -> bool {
        EnablingConditionDispatched::is_enabled(self, state)
    }
}

// bincode decoding fails with: "Bincode does not support Deserializer::deserialize_identifier".
// So use messagepack instead, which is smaller but slower.

impl storage::persistent::Encoder for Action {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        rmp_serde::to_vec(self).map_err(|_| SchemaError::EncodeError)
    }
}

impl storage::persistent::Decoder for Action {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        rmp_serde::from_slice(bytes).map_err(|_| SchemaError::DecodeError)
    }
}

impl<'a> From<&'a ActionWithMeta> for ActionKind {
    fn from(action: &'a ActionWithMeta) -> ActionKind {
        action.action.kind()
    }
}

impl From<ActionWithMeta> for ActionKind {
    fn from(action: ActionWithMeta) -> ActionKind {
        action.action.kind()
    }
}
