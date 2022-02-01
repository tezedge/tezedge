#![cfg_attr(test, feature(no_coverage))]

use fuzzcheck::{DefaultMutator, SerdeSerializer};
use once_cell::sync::Lazy;
use redux_rs::{ActionId, ActionWithMeta, EnablingCondition, SafetyCondition};
use serde::{Deserialize, Serialize};
use shell_automaton::action::Action;
use shell_automaton::fuzzing::state_singleton::FUZZER_STATE;
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
    storage::{
        request::{
            //StorageRequestCreateAction,
            //StorageRequestErrorAction,
            StorageRequestFinishAction,
            StorageRequestInitAction,
            StorageRequestPendingAction,
            //StorageRequestSuccessAction,
            //StorageResponseReceivedAction
        },
        state_snapshot::create::StorageStateSnapshotCreateInitAction,
    },
};
use std::convert::TryInto;
use std::env;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::sync::Mutex;
use std::time::SystemTime;
use storage::persistent::codec::Decoder;

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum AllActionsTest {
    TestControl(ControlActionTest),
    TestPeersDnsAction(PeersDnsLookupActionTest),
    TestPeerActions(PeerActionTest),
    TestMioEvents(MioActionTest),
    TestStorage(StorageActionTest),
}

impl AllActionsTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestControl(a) => a.to_action(),
            Self::TestPeersDnsAction(a) => a.to_action(),
            Self::TestPeerActions(a) => a.to_action(),
            Self::TestMioEvents(a) => a.to_action(),
            Self::TestStorage(a) => a.to_action(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum StorageActionTest {
    // StorageRequestPayload case in particular is complex
    //TestStorageRequestCreateAction(StorageRequestCreateAction),
    TestStorageRequestInitAction(StorageRequestInitAction),
    TestStorageRequestPendingAction(StorageRequestPendingAction),
    // Needs ActionId wrapper
    //TestStorageResponseReceivedAction(StorageResponseReceivedAction),
    //TestStorageRequestErrorAction(StorageRequestErrorAction),
    //TestStorageRequestSuccessAction(StorageRequestSuccessAction),
    TestStorageRequestFinishAction(StorageRequestFinishAction),
    TestStorageStateSnapshotCreateInitAction(StorageStateSnapshotCreateInitAction),
    // Needs ActionId wrapper
    //TestStorageStateSnapshotCreatePendingAction(StorageStateSnapshotCreatePendingAction),
    //TestStorageStateSnapshotCreateErrorAction(StorageStateSnapshotCreateErrorAction),
    //TestStorageStateSnapshotCreateSuccessAction(StorageStateSnapshotCreateSuccessAction)
}

impl StorageActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestStorageRequestInitAction(a) => a.into(),
            Self::TestStorageRequestPendingAction(a) => a.into(),
            Self::TestStorageRequestFinishAction(a) => a.into(),
            Self::TestStorageStateSnapshotCreateInitAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PeerActionTest {
    //TestPeersGraylistAction(PeersGraylistActionTest),
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
            //Self::TestPeersGraylistAction(a) => a.to_action(),
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
enum MioActionTest {
    TestP2pServerEvent(P2pServerEvent),
    TestP2pPeerEvent(P2pPeerEvent),
    TestWakeupEvent(WakeupEvent),
}

impl MioActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestP2pServerEvent(a) => a.into(),
            Self::TestP2pPeerEvent(a) => a.into(),
            Self::TestWakeupEvent(a) => a.into(),
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

fn is_action_enabled(action: Action, state: &State) -> bool {
    match action {
        Action::PausedLoopsAdd(a) => a.is_enabled(state),
        Action::PausedLoopsResumeAll(a) => a.is_enabled(state),
        Action::PausedLoopsResumeNextInit(a) => a.is_enabled(state),
        Action::PausedLoopsResumeNextSuccess(a) => a.is_enabled(state),
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
        Action::StorageRequestInit(a) => a.is_enabled(state),
        Action::StorageRequestPending(a) => a.is_enabled(state),
        Action::StorageRequestFinish(a) => a.is_enabled(state),
        Action::StorageStateSnapshotCreateInit(a) => a.is_enabled(state),
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

fn action_test_mio(action_test: &MioActionTest) -> bool {
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
fn test_mio() {
    fuzzcheck::fuzz_test(action_test_mio)
        .mutator(MioActionTest::default_mutator())
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
