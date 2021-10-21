use std::net::SocketAddr;

use derive_more::From;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use storage::persistent::SchemaError;

use crate::event::{P2pPeerEvent, P2pServerEvent, WakeupEvent};

use crate::peer::binary_message::read::*;
use crate::peer::binary_message::write::*;
use crate::peer::chunk::read::*;
use crate::peer::chunk::write::*;
use crate::peer::message::read::*;
use crate::peer::message::write::*;
use crate::peer::{
    PeerErrorAction, PeerReadWouldBlockAction, PeerTryReadAction, PeerTryWriteAction,
    PeerWriteWouldBlockAction,
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

use crate::peers::add::multi::PeersAddMultiAction;
use crate::peers::add::PeersAddIncomingPeerAction;
use crate::peers::check::timeouts::{
    PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction, PeersCheckTimeoutsSuccessAction,
};
use crate::peers::dns_lookup::{
    PeersDnsLookupCleanupAction, PeersDnsLookupErrorAction, PeersDnsLookupInitAction,
    PeersDnsLookupSuccessAction,
};
use crate::peers::remove::PeersRemoveAction;

use crate::state::ActionIdWithKind;
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
#[enum_kind(
    ActionKind,
    derive(strum_macros::Display, Serialize, Deserialize, Hash)
)]
#[serde(tag = "kind", content = "content")]
pub enum Action {
    Init,

    PeersDnsLookupInit(PeersDnsLookupInitAction),
    PeersDnsLookupError(PeersDnsLookupErrorAction),
    PeersDnsLookupSuccess(PeersDnsLookupSuccessAction),
    PeersDnsLookupCleanup(PeersDnsLookupCleanupAction),

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

    MioTimeoutEvent,
    MioIdleEvent,
    P2pServerEvent(P2pServerEvent),
    P2pPeerEvent(P2pPeerEvent),
    WakeupEvent(WakeupEvent),

    PeerTryWrite(PeerTryWriteAction),
    PeerTryRead(PeerTryReadAction),
    PeerReadWouldBlock(PeerReadWouldBlockAction),
    PeerWriteWouldBlock(PeerWriteWouldBlockAction),

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

    PeerError(PeerErrorAction),

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

    // Action to reset dispatch recursion counter
    DispatchRecursionReset,
    DispatchRecursionIncrement,
    DispatchRecursionLimitExceeded(DispatchRecursionLimitExceededAction),
}

impl Action {
    #[inline(always)]
    pub fn kind(&self) -> ActionKind {
        ActionKind::from(self)
    }

    pub fn get_peer_address(&self) -> Option<&SocketAddr> {
        match self {
            Action::PeersAddIncomingPeer(PeersAddIncomingPeerAction { address, .. })
            | Action::PeersRemove(PeersRemoveAction { address, .. })
            | Action::PeerConnectionIncomingRejected(PeerConnectionIncomingRejectedAction {
                address,
                ..
            })
            | Action::PeerConnectionIncomingAcceptSuccess(
                PeerConnectionIncomingAcceptSuccessAction { address, .. },
            )
            | Action::PeerConnectionIncomingError(PeerConnectionIncomingErrorAction {
                address,
                ..
            })
            | Action::PeerConnectionIncomingSuccess(PeerConnectionIncomingSuccessAction {
                address,
                ..
            })
            | Action::PeerConnectionOutgoingInit(PeerConnectionOutgoingInitAction {
                address,
                ..
            })
            | Action::PeerConnectionOutgoingPending(PeerConnectionOutgoingPendingAction {
                address,
                ..
            })
            | Action::PeerConnectionOutgoingError(PeerConnectionOutgoingErrorAction {
                address,
                ..
            })
            | Action::PeerConnectionOutgoingSuccess(PeerConnectionOutgoingSuccessAction {
                address,
                ..
            })
            | Action::PeerConnectionClosed(PeerConnectionClosedAction { address, .. })
            | Action::PeerDisconnect(PeerDisconnectAction { address, .. })
            | Action::PeerDisconnected(PeerDisconnectedAction { address, .. })
            | Action::P2pPeerEvent(P2pPeerEvent { address, .. })
            | Action::PeerTryWrite(PeerTryWriteAction { address, .. })
            | Action::PeerTryRead(PeerTryReadAction { address, .. })
            | Action::PeerReadWouldBlock(PeerReadWouldBlockAction { address, .. })
            | Action::PeerWriteWouldBlock(PeerWriteWouldBlockAction { address, .. })
            | Action::PeerChunkReadInit(PeerChunkReadInitAction { address, .. })
            | Action::PeerChunkReadPart(PeerChunkReadPartAction { address, .. })
            | Action::PeerChunkReadDecrypt(PeerChunkReadDecryptAction { address, .. })
            | Action::PeerChunkReadReady(PeerChunkReadReadyAction { address, .. })
            | Action::PeerChunkReadError(PeerChunkReadErrorAction { address, .. })
            | Action::PeerChunkWriteSetContent(PeerChunkWriteSetContentAction {
                address, ..
            })
            | Action::PeerChunkWriteEncryptContent(PeerChunkWriteEncryptContentAction {
                address,
                ..
            })
            | Action::PeerChunkWriteCreateChunk(PeerChunkWriteCreateChunkAction {
                address, ..
            })
            | Action::PeerChunkWritePart(PeerChunkWritePartAction { address, .. })
            | Action::PeerChunkWriteReady(PeerChunkWriteReadyAction { address, .. })
            | Action::PeerChunkWriteError(PeerChunkWriteErrorAction { address, .. })
            | Action::PeerBinaryMessageReadInit(PeerBinaryMessageReadInitAction {
                address, ..
            })
            | Action::PeerBinaryMessageReadChunkReady(PeerBinaryMessageReadChunkReadyAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageReadSizeReady(PeerBinaryMessageReadSizeReadyAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageReadReady(PeerBinaryMessageReadReadyAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageReadError(PeerBinaryMessageReadErrorAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageWriteSetContent(PeerBinaryMessageWriteSetContentAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageWriteNextChunk(PeerBinaryMessageWriteNextChunkAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageWriteReady(PeerBinaryMessageWriteReadyAction {
                address,
                ..
            })
            | Action::PeerBinaryMessageWriteError(PeerBinaryMessageWriteErrorAction {
                address,
                ..
            })
            | Action::PeerMessageReadInit(PeerMessageReadInitAction { address, .. })
            | Action::PeerMessageReadSuccess(PeerMessageReadSuccessAction { address, .. })
            | Action::PeerMessageWriteNext(PeerMessageWriteNextAction { address, .. })
            | Action::PeerMessageWriteInit(PeerMessageWriteInitAction { address, .. })
            | Action::PeerMessageWriteSuccess(PeerMessageWriteSuccessAction { address, .. })
            | Action::PeerHandshakingInit(PeerHandshakingInitAction { address, .. })
            | Action::PeerHandshakingConnectionMessageInit(
                PeerHandshakingConnectionMessageInitAction { address, .. },
            )
            | Action::PeerHandshakingConnectionMessageEncode(
                PeerHandshakingConnectionMessageEncodeAction { address, .. },
            )
            | Action::PeerHandshakingConnectionMessageWrite(
                PeerHandshakingConnectionMessageWriteAction { address, .. },
            )
            | Action::PeerHandshakingConnectionMessageRead(
                PeerHandshakingConnectionMessageReadAction { address, .. },
            )
            | Action::PeerHandshakingConnectionMessageDecode(
                PeerHandshakingConnectionMessageDecodeAction { address, .. },
            )
            | Action::PeerHandshakingEncryptionInit(PeerHandshakingEncryptionInitAction {
                address,
                ..
            })
            | Action::PeerHandshakingMetadataMessageInit(
                PeerHandshakingMetadataMessageInitAction { address, .. },
            )
            | Action::PeerHandshakingMetadataMessageEncode(
                PeerHandshakingMetadataMessageEncodeAction { address, .. },
            )
            | Action::PeerHandshakingMetadataMessageWrite(
                PeerHandshakingMetadataMessageWriteAction { address, .. },
            )
            | Action::PeerHandshakingMetadataMessageRead(
                PeerHandshakingMetadataMessageReadAction { address, .. },
            )
            | Action::PeerHandshakingMetadataMessageDecode(
                PeerHandshakingMetadataMessageDecodeAction { address, .. },
            )
            | Action::PeerHandshakingAckMessageInit(PeerHandshakingAckMessageInitAction {
                address,
                ..
            })
            | Action::PeerHandshakingAckMessageEncode(PeerHandshakingAckMessageEncodeAction {
                address,
                ..
            })
            | Action::PeerHandshakingAckMessageWrite(PeerHandshakingAckMessageWriteAction {
                address,
                ..
            })
            | Action::PeerHandshakingAckMessageRead(PeerHandshakingAckMessageReadAction {
                address,
                ..
            })
            | Action::PeerHandshakingAckMessageDecode(PeerHandshakingAckMessageDecodeAction {
                address,
                ..
            })
            | Action::PeerHandshakingError(PeerHandshakingErrorAction { address, .. })
            | Action::PeerHandshakingFinish(PeerHandshakingFinishAction { address, .. }) => {
                Some(address)
            }
            _ => None,
        }
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

impl<'a> From<&'a ActionWithId<Action>> for ActionKind {
    fn from(action: &'a ActionWithId<Action>) -> ActionKind {
        action.action.kind()
    }
}

impl From<ActionWithId<Action>> for ActionKind {
    fn from(action: ActionWithId<Action>) -> ActionKind {
        action.action.kind()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DispatchRecursionLimitExceededAction {
    pub peer_address: Option<SocketAddr>,
    pub action: ActionIdWithKind,
    pub backtrace: Vec<ActionIdWithKind>,
}
