// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use storage::persistent::SchemaError;

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
use crate::peer::{
    PeerTryReadLoopFinishAction, PeerTryReadLoopStartAction, PeerTryWriteLoopFinishAction,
    PeerTryWriteLoopStartAction,
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

use crate::mempool::mempool_actions::*;
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
use crate::peers::remove::PeersRemoveAction;

use crate::protocol::protocol_actions::*;

use crate::rights::{
    RightsEndorsingRightsBlockHeaderReadyAction, RightsEndorsingRightsCalculateAction,
    RightsEndorsingRightsCycleDataReadyAction, RightsEndorsingRightsCycleErasReadyAction,
    RightsEndorsingRightsCycleReadyAction, RightsEndorsingRightsErrorAction,
    RightsEndorsingRightsGetBlockHeaderAction, RightsEndorsingRightsGetCycleAction,
    RightsEndorsingRightsGetCycleDataAction, RightsEndorsingRightsGetCycleErasAction,
    RightsEndorsingRightsGetProtocolConstantsAction, RightsEndorsingRightsGetProtocolHashAction,
    RightsEndorsingRightsProtocolConstantsReadyAction,
    RightsEndorsingRightsProtocolHashReadyAction, RightsEndorsingRightsReadyAction,
    RightsGetEndorsingRightsAction,
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

pub use redux_rs::{ActionId, EnablingCondition};

pub type ActionWithMeta = redux_rs::ActionWithMeta<Action>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InitAction {}

impl EnablingCondition<State> for InitAction {
    fn is_enabled(&self, _: &State) -> bool {
        false
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MioWaitForEventsAction {}

impl EnablingCondition<State> for MioWaitForEventsAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MioTimeoutEvent {}

impl EnablingCondition<State> for MioTimeoutEvent {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(
    EnumKind,
    strum_macros::AsRefStr,
    strum_macros::IntoStaticStr,
    From,
    Serialize,
    Deserialize,
    Debug,
    Clone,
)]
#[enum_kind(
    ActionKind,
    derive(
        strum_macros::EnumIter,
        strum_macros::Display,
        Serialize,
        Deserialize,
        Hash
    )
)]
#[serde(tag = "kind", content = "content")]
pub enum Action {
    Init(InitAction),

    PausedLoopsAdd(PausedLoopsAddAction),
    PausedLoopsResumeAll(PausedLoopsResumeAllAction),
    PausedLoopsResumeNextInit(PausedLoopsResumeNextInitAction),
    PausedLoopsResumeNextSuccess(PausedLoopsResumeNextSuccessAction),

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

    ProtocolConstructStatelessPrevalidatorStart(ProtocolConstructStatelessPrevalidatorStartAction),
    ProtocolConstructStatelessPrevalidatorDone(ProtocolConstructStatelessPrevalidatorDoneAction),
    ProtocolConstructStatefulPrevalidatorStart(ProtocolConstructStatefulPrevalidatorStartAction),
    ProtocolConstructStatefulPrevalidatorDone(ProtocolConstructStatefulPrevalidatorDoneAction),
    ProtocolValidateOperationStart(ProtocolValidateOperationStartAction),
    ProtocolValidateOperationDone(ProtocolValidateOperationDoneAction),

    MempoolRecvDone(MempoolRecvDoneAction),
    MempoolGetOperations(MempoolGetOperationsAction),
    MempoolMarkOperationsAsPending(MempoolMarkOperationsAsPendingAction),
    MempoolOperationRecvDone(MempoolOperationRecvDoneAction),
    MempoolOperationInject(MempoolOperationInjectAction),
    MempoolValidateStart(MempoolValidateStartAction),
    MempoolValidateWaitPrevalidator(MempoolValidateWaitPrevalidatorAction),
    MempoolRpcRespond(MempoolRpcRespondAction),
    MempoolRegisterOperationsStream(MempoolRegisterOperationsStreamAction),
    MempoolUnregisterOperationsStreams(MempoolUnregisterOperationsStreamsAction),
    MempoolSend(MempoolSendAction),
    MempoolSendValidated(MempoolSendValidatedAction),
    MempoolAskCurrentHead(MempoolAskCurrentHeadAction),
    MempoolBroadcast(MempoolBroadcastAction),
    MempoolBroadcastDone(MempoolBroadcastDoneAction),
    MempoolCleanupWaitPrevalidator(MempoolCleanupWaitPrevalidatorAction),
    MempoolRemoveAppliedOperations(MempoolRemoveAppliedOperationsAction),
    MempoolGetPendingOperations(MempoolGetPendingOperationsAction),
    MempoolFlush(MempoolFlushAction),

    BlockApplied(BlockAppliedAction),

    RightsGetEndorsingRights(RightsGetEndorsingRightsAction),
    RightsEndorsingRightsGetBlockHeader(RightsEndorsingRightsGetBlockHeaderAction),
    RightsEndorsingRightsBlockHeaderReady(RightsEndorsingRightsBlockHeaderReadyAction),
    RightsEndorsingRightsGetProtocolHash(RightsEndorsingRightsGetProtocolHashAction),
    RightsEndorsingRightsProtocolHashReady(RightsEndorsingRightsProtocolHashReadyAction),
    RightsEndorsingRightsGetProtocolConstants(RightsEndorsingRightsGetProtocolConstantsAction),
    RightsEndorsingRightsProtocolConstantsReady(RightsEndorsingRightsProtocolConstantsReadyAction),
    RightsEndorsingRightsGetCycleEras(RightsEndorsingRightsGetCycleErasAction),
    RightsEndorsingRightsCycleErasReady(RightsEndorsingRightsCycleErasReadyAction),
    RightsEndorsingRightsGetCycle(RightsEndorsingRightsGetCycleAction),
    RightsEndorsingRightsCycleReady(RightsEndorsingRightsCycleReadyAction),
    RightsEndorsingRightsGetCycleData(RightsEndorsingRightsGetCycleDataAction),
    RightsEndorsingRightsCycleDataReady(RightsEndorsingRightsCycleDataReadyAction),
    RightsEndorsingRightsCalculate(RightsEndorsingRightsCalculateAction),
    RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction),
    RightsEndorsingRightsError(RightsEndorsingRightsErrorAction),

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
}

impl Action {
    #[inline(always)]
    pub fn kind(&self) -> ActionKind {
        ActionKind::from(self)
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
