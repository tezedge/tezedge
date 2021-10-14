use derive_more::From;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use storage::persistent::SchemaError;

use crate::event::{P2pPeerEvent, P2pServerEvent, WakeupEvent};
use crate::peer::connection::closed::PeerConnectionClosedAction;
use crate::peer::connection::incoming::accept::*;
use crate::peer::connection::incoming::PeerConnectionIncomingSuccessAction;

use crate::peer::binary_message::read::*;
use crate::peer::binary_message::write::*;
use crate::peer::chunk::read::*;
use crate::peer::chunk::write::*;
use crate::peer::message::read::*;
use crate::peer::message::write::*;

use crate::peer::connection::outgoing::{
    PeerConnectionOutgoingErrorAction, PeerConnectionOutgoingInitAction,
    PeerConnectionOutgoingPendingAction, PeerConnectionOutgoingRandomInitAction,
    PeerConnectionOutgoingSuccessAction,
};
use crate::peer::disconnection::{PeerDisconnectAction, PeerDisconnectedAction};
use crate::peer::handshaking::*;

use crate::peer::{PeerTryReadAction, PeerTryWriteAction};
use crate::peers::add::multi::PeersAddMultiAction;
use crate::peers::add::PeersAddIncomingPeerAction;
use crate::peers::dns_lookup::{
    PeersDnsLookupCleanupAction, PeersDnsLookupErrorAction, PeersDnsLookupInitAction,
    PeersDnsLookupSuccessAction,
};
use crate::peers::remove::PeersRemoveAction;
use crate::storage::block_header::put::{
    StorageBlockHeaderPutNextInitAction, StorageBlockHeaderPutNextPendingAction,
    StorageBlockHeadersPutAction,
};
use crate::storage::request::{
    StorageRequestCreateAction, StorageRequestErrorAction, StorageRequestFinishAction,
    StorageRequestInitAction, StorageRequestPendingAction, StorageRequestSuccessAction,
};
use crate::storage::state_snapshot::create::StorageStateSnapshotCreateAction;

pub use redux_rs::{ActionId, ActionWithId};

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
#[enum_kind(ActionType, derive(Serialize, Deserialize, Hash))]
#[serde(tag = "type", content = "content")]
pub enum Action {
    PeersDnsLookupInit(PeersDnsLookupInitAction),
    PeersDnsLookupError(PeersDnsLookupErrorAction),
    PeersDnsLookupSuccess(PeersDnsLookupSuccessAction),
    PeersDnsLookupCleanup(PeersDnsLookupCleanupAction),

    PeersAddIncomingPeer(PeersAddIncomingPeerAction),
    PeersAddMulti(PeersAddMultiAction),
    PeersRemove(PeersRemoveAction),

    PeerConnectionIncomingAccept(PeerConnectionIncomingAcceptAction),
    PeerConnectionIncomingAcceptError(PeerConnectionIncomingAcceptErrorAction),
    PeerConnectionIncomingAcceptSuccess(PeerConnectionIncomingAcceptSuccessAction),

    PeerConnectionIncomingSuccess(PeerConnectionIncomingSuccessAction),

    PeerConnectionOutgoingRandomInit(PeerConnectionOutgoingRandomInitAction),
    PeerConnectionOutgoingInit(PeerConnectionOutgoingInitAction),
    PeerConnectionOutgoingPending(PeerConnectionOutgoingPendingAction),
    PeerConnectionOutgoingError(PeerConnectionOutgoingErrorAction),
    PeerConnectionOutgoingSuccess(PeerConnectionOutgoingSuccessAction),

    PeerConnectionClosed(PeerConnectionClosedAction),

    PeerDisconnect(PeerDisconnectAction),
    PeerDisconnected(PeerDisconnectedAction),

    P2pServerEvent(P2pServerEvent),
    P2pPeerEvent(P2pPeerEvent),
    WakeupEvent(WakeupEvent),

    PeerTryWrite(PeerTryWriteAction),
    PeerTryRead(PeerTryReadAction),

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
    PeerMessageReadSuccess(PeerMessageReadSuccessAction),

    PeerMessageWriteNext(PeerMessageWriteNextAction),
    PeerMessageWriteInit(PeerMessageWriteInitAction),
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

    StorageBlockHeadersPut(StorageBlockHeadersPutAction),
    StorageBlockHeaderPutNextInit(StorageBlockHeaderPutNextInitAction),
    StorageBlockHeaderPutNextPending(StorageBlockHeaderPutNextPendingAction),

    StorageStateSnapshotCreate(StorageStateSnapshotCreateAction),

    StorageRequestCreate(StorageRequestCreateAction),
    StorageRequestInit(StorageRequestInitAction),
    StorageRequestPending(StorageRequestPendingAction),
    StorageRequestError(StorageRequestErrorAction),
    StorageRequestSuccess(StorageRequestSuccessAction),
    StorageRequestFinish(StorageRequestFinishAction),
}

impl Action {
    #[inline(always)]
    pub fn action_type(&self) -> ActionType {
        ActionType::from(self)
    }
}

// bincode decoding fails with: "Bincode does not support Deserializer::deserialize_identifier".
// So use json instead, which works.

// impl BincodeEncoded for Action {
//     fn decode(bytes: &[u8]) -> Result<Self, storage::persistent::SchemaError> {
//         // here it errors.
//         Ok(dbg!(bincode::deserialize(bytes)).unwrap())
//     }

//     fn encode(&self) -> Result<Vec<u8>, storage::persistent::SchemaError> {
//         Ok(bincode::serialize::<Self>(self).unwrap())
//     }
// }

impl storage::persistent::Encoder for Action {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        serde_json::to_vec(self).map_err(|_| SchemaError::EncodeError)
    }
}

impl storage::persistent::Decoder for Action {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        serde_json::from_slice(bytes).map_err(|_| SchemaError::DecodeError)
    }
}

impl<'a> From<&'a ActionWithId<Action>> for ActionType {
    fn from(action: &'a ActionWithId<Action>) -> ActionType {
        action.action.action_type()
    }
}

impl From<ActionWithId<Action>> for ActionType {
    fn from(action: ActionWithId<Action>) -> ActionType {
        action.action.action_type()
    }
}
