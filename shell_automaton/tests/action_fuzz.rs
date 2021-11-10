#![cfg_attr(test, feature(no_coverage))]
use serde::{Deserialize, Serialize};
use fuzzcheck::{DefaultMutator, SerdeSerializer};
use shell_automaton::{event::{P2pPeerEvent, P2pServerEvent, WakeupEvent}, paused_loops::{
        PausedLoopsAddAction,
        PausedLoopsResumeAllAction,
        PausedLoopsResumeNextInitAction,
        PausedLoopsResumeNextSuccessAction
    }, peer::{
        PeerTryReadLoopFinishAction,
        PeerTryReadLoopStartAction,
        PeerTryWriteLoopFinishAction,
        PeerTryWriteLoopStartAction,
        binary_message::{
            read::{
                PeerBinaryMessageReadChunkReadyAction,
                PeerBinaryMessageReadErrorAction,
                PeerBinaryMessageReadInitAction,
                PeerBinaryMessageReadReadyAction,
                PeerBinaryMessageReadSizeReadyAction
            },
            write::{
                PeerBinaryMessageWriteErrorAction,
                PeerBinaryMessageWriteNextChunkAction,
                PeerBinaryMessageWriteReadyAction,
                PeerBinaryMessageWriteSetContentAction
            }
        },
        chunk::{
            read::{
                PeerChunkReadDecryptAction,
                PeerChunkReadErrorAction,
                PeerChunkReadInitAction,
                PeerChunkReadPartAction,
                PeerChunkReadReadyAction
            },
            write::{
                PeerChunkWriteCreateChunkAction,
                PeerChunkWriteEncryptContentAction,
                PeerChunkWriteErrorAction,
                PeerChunkWritePartAction,
                PeerChunkWriteReadyAction,
                PeerChunkWriteSetContentAction
            }
        },
        connection::{
            closed::PeerConnectionClosedAction,
            incoming::{
                PeerConnectionIncomingErrorAction,
                PeerConnectionIncomingSuccessAction,
                accept::{
                    PeerConnectionIncomingAcceptAction,
                    PeerConnectionIncomingAcceptErrorAction,
                    PeerConnectionIncomingAcceptSuccessAction,
                    PeerConnectionIncomingRejectedAction
                }
            },
            outgoing::{
                PeerConnectionOutgoingErrorAction,
                PeerConnectionOutgoingInitAction,
                PeerConnectionOutgoingPendingAction,
                PeerConnectionOutgoingRandomInitAction,
                PeerConnectionOutgoingSuccessAction
            }
        },
        disconnection::{ PeerDisconnectAction, PeerDisconnectedAction },
        handshaking::{
            PeerHandshakingAckMessageDecodeAction,
            PeerHandshakingAckMessageEncodeAction,
            PeerHandshakingAckMessageInitAction,
            PeerHandshakingAckMessageReadAction,
            PeerHandshakingAckMessageWriteAction,
            PeerHandshakingConnectionMessageDecodeAction,
            PeerHandshakingConnectionMessageEncodeAction,
            PeerHandshakingConnectionMessageInitAction,
            PeerHandshakingConnectionMessageReadAction,
            PeerHandshakingConnectionMessageWriteAction,
            PeerHandshakingEncryptionInitAction,
            PeerHandshakingErrorAction,
            PeerHandshakingFinishAction,
            PeerHandshakingInitAction,
            PeerHandshakingMetadataMessageDecodeAction,
            PeerHandshakingMetadataMessageEncodeAction,
            PeerHandshakingMetadataMessageInitAction,
            PeerHandshakingMetadataMessageReadAction,
            PeerHandshakingMetadataMessageWriteAction
        },
        message::{
            read::{
                PeerMessageReadErrorAction,
                PeerMessageReadInitAction,
                PeerMessageReadSuccessAction
            },
            write::{
                PeerMessageWriteErrorAction,
                PeerMessageWriteInitAction,
                PeerMessageWriteNextAction,
                PeerMessageWriteSuccessAction
            }
        }
    }, peers::{add::{PeersAddIncomingPeerAction, multi::PeersAddMultiAction}, check::timeouts::{PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction, PeersCheckTimeoutsSuccessAction}, dns_lookup::{
            PeersDnsLookupCleanupAction,
            PeersDnsLookupErrorAction,
            PeersDnsLookupSuccessAction,
            PeersDnsLookupInitAction
        }, graylist::{PeersGraylistAddressAction, PeersGraylistIpAddAction, PeersGraylistIpAddedAction, PeersGraylistIpRemoveAction, PeersGraylistIpRemovedAction}, remove::PeersRemoveAction}, storage::{
        request::{
            StorageRequestCreateAction,
            StorageRequestErrorAction,
            StorageRequestFinishAction,
            StorageRequestInitAction,
            StorageRequestPendingAction,
            StorageRequestSuccessAction,
            StorageResponseReceivedAction
        },
        state_snapshot::create::{
            StorageStateSnapshotCreateErrorAction,
            StorageStateSnapshotCreateInitAction,
            StorageStateSnapshotCreatePendingAction,
            StorageStateSnapshotCreateSuccessAction
        }
    }};

#[derive(fuzzcheck::DefaultMutator)]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum ActionTest {
    TestPausedLoopsAddAction(PausedLoopsAddAction),
    TestPausedLoopsResumeAllAction(PausedLoopsResumeAllAction),
    TestPausedLoopsResumeNextInitAction(PausedLoopsResumeNextInitAction),
    TestPausedLoopsResumeNextSuccessAction(PausedLoopsResumeNextSuccessAction),
    TestPeersDnsLookupInitAction(PeersDnsLookupInitAction),
    TestPeersDnsLookupErrorAction(PeersDnsLookupErrorAction),
    TestPeersDnsLookupSuccessAction(PeersDnsLookupSuccessAction),
    TestPeersDnsLookupCleanupAction(PeersDnsLookupCleanupAction),
    TestPeersGraylistAddressAction(PeersGraylistAddressAction),
    TestPeersPlaylistIpAddedAction(PeersGraylistIpAddAction),
    TestPeersGraylistIpAddedAction(PeersGraylistIpAddedAction),
    TestPeersGraylistIpRemoveAction(PeersGraylistIpRemoveAction),
    TestPeersGraylistIpRemovedAction(PeersGraylistIpRemovedAction),
    TestPeersAddIncomingPeerAction(PeersAddIncomingPeerAction),
    TestPeersAddMultiAction(PeersAddMultiAction),
    TestPeersRemoveAction(PeersRemoveAction),
    TestPeersCheckTimeoutsInitAction(PeersCheckTimeoutsInitAction),
    TestPeersCheckTimeoutsSuccessAction(PeersCheckTimeoutsSuccessAction),
    TestPeersCheckTimeoutsCleanupAction(PeersCheckTimeoutsCleanupAction),
    TestPeerConnectionIncomingAcceptAction(PeerConnectionIncomingAcceptAction),
    TestPeerConnectionIncomingAcceptErrorAction(PeerConnectionIncomingAcceptErrorAction),
    TestPeerConnectionIncomingRejectedAction(PeerConnectionIncomingRejectedAction),
    TestPeerConnectionIncomingAcceptSuccessAction(PeerConnectionIncomingAcceptSuccessAction),
    TestPeerConnectionIncomingErrorAction(PeerConnectionIncomingErrorAction),
    TestPeerConnectionIncomingSuccessAction(PeerConnectionIncomingSuccessAction),
    TestPeerConnectionOutgoingRandomInitAction(PeerConnectionOutgoingRandomInitAction),
    TestPeerConnectionOutgoingInitAction(PeerConnectionOutgoingInitAction),
    TestPeerConnectionOutgoingPendingAction(PeerConnectionOutgoingPendingAction),
    TestPeerConnectionOutgoingErrorAction(PeerConnectionOutgoingErrorAction),
    TestPeerConnectionOutgoingSuccessAction(PeerConnectionOutgoingSuccessAction),
    TestPeerConnectionClosedAction(PeerConnectionClosedAction),
    TestPeerDisconnectAction(PeerDisconnectAction),
    TestPeerDisconnectedAction(PeerDisconnectedAction),
    TestP2pServerEvent(P2pServerEvent),
    TestP2pPeerEvent(P2pPeerEvent),
    TestWakeupEvent(WakeupEvent),
    TestPeerTryWriteLoopStartAction(PeerTryWriteLoopStartAction),
    TestPeerTryWriteLoopFinishAction(PeerTryWriteLoopFinishAction),
    TestPeerTryReadLoopStartAction(PeerTryReadLoopStartAction),
    TestPeerTryReadLoopFinishAction(PeerTryReadLoopFinishAction),
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
    TestPeerMessageReadInitAction(PeerMessageReadInitAction),
    TestPeerMessageReadErrorAction(PeerMessageReadErrorAction),
    TestPeerMessageReadSuccessAction(PeerMessageReadSuccessAction),
    TestPeerMessageWriteNextAction(PeerMessageWriteNextAction),
    TestPeerMessageWriteInitAction(PeerMessageWriteInitAction),
    TestPeerMessageWriteErrorAction(PeerMessageWriteErrorAction),
    TestPeerMessageWriteSuccessAction(PeerMessageWriteSuccessAction),
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


fn testfun(test: &ActionTest) {
    println!("test {:?}", test);
}
#[cfg(test)]
#[test]
fn test_function_shouldn_t_crash() {
    let _ = fuzzcheck::fuzz_test(testfun) 
        .mutator(ActionTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor() 
        .default_pool() 
        .arguments_from_cargo_fuzzcheck()
        .stop_after_first_test_failure(true)
        .launch();
        
}
