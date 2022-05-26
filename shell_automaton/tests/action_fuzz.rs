// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "fuzzing")]
#![feature(backtrace)]
#![feature(alloc_error_hook)]
#![cfg_attr(test, feature(no_coverage))]
use ::storage::persistent::Encoder;
use fuzzcheck::{DefaultMutator, SerdeSerializer};
use once_cell::sync::Lazy;
use redux_rs::{ActionId, ActionWithMeta, SafetyCondition};
use serde::{Deserialize, Serialize};
use shell_automaton::action::{Action, InitAction};
use shell_automaton::block_applier;
use shell_automaton::current_head_precheck;
use shell_automaton::fuzzing::state_singleton::FUZZER_STATE;
use shell_automaton::mempool::mempool_actions;
use shell_automaton::mempool::validator as mempool_validator;
use shell_automaton::peers::init::PeersInitAction;
use shell_automaton::prechecker::prechecker_actions;
use shell_automaton::protocol_runner;
use shell_automaton::rights::{
    cycle_delegates::rights_cycle_delegates_actions, cycle_eras::rights_cycle_eras_actions,
    rights_actions,
};
use shell_automaton::shutdown::ShutdownInitAction;
use shell_automaton::shutdown::ShutdownPendingAction;
use shell_automaton::shutdown::ShutdownSuccessAction;
use shell_automaton::stats::current_head::stats_current_head_actions;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::panic::catch_unwind;
use std::panic::AssertUnwindSafe;
use std::sync::RwLock;
//use shell_automaton::storage::request::StorageRequestSuccessAction;
use shell_automaton::MioWaitForEventsAction;

use shell_automaton::MioTimeoutEvent;
use shell_automaton::State;
use shell_automaton::{
    bootstrap::{
        BootstrapCheckTimeoutsInitAction, BootstrapErrorAction, BootstrapFinishedAction,
        BootstrapFromPeerCurrentHeadAction, BootstrapInitAction,
        BootstrapPeerBlockHeaderGetFinishAction, BootstrapPeerBlockHeaderGetInitAction,
        BootstrapPeerBlockHeaderGetPendingAction, BootstrapPeerBlockHeaderGetSuccessAction,
        BootstrapPeerBlockHeaderGetTimeoutAction, BootstrapPeerBlockOperationsGetPendingAction,
        BootstrapPeerBlockOperationsGetRetryAction, BootstrapPeerBlockOperationsGetSuccessAction,
        BootstrapPeerBlockOperationsGetTimeoutAction, BootstrapPeerBlockOperationsReceivedAction,
        BootstrapPeerCurrentBranchReceivedAction, BootstrapPeersBlockHeadersGetInitAction,
        BootstrapPeersBlockHeadersGetPendingAction, BootstrapPeersBlockHeadersGetSuccessAction,
        BootstrapPeersBlockOperationsGetInitAction, BootstrapPeersBlockOperationsGetNextAction,
        BootstrapPeersBlockOperationsGetNextAllAction,
        BootstrapPeersBlockOperationsGetPendingAction,
        BootstrapPeersBlockOperationsGetSuccessAction, BootstrapPeersConnectPendingAction,
        BootstrapPeersConnectSuccessAction, BootstrapPeersMainBranchFindInitAction,
        BootstrapPeersMainBranchFindPendingAction, BootstrapPeersMainBranchFindSuccessAction,
        BootstrapScheduleBlockForApplyAction, BootstrapScheduleBlocksForApplyAction,
    },
    current_head::{
        CurrentHeadRehydrateErrorAction, CurrentHeadRehydrateInitAction,
        CurrentHeadRehydratePendingAction, CurrentHeadRehydrateSuccessAction,
        CurrentHeadRehydratedAction, CurrentHeadUpdateAction,
    },
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
        remote_requests::{
            block_header_get::{
                PeerRemoteRequestsBlockHeaderGetEnqueueAction,
                PeerRemoteRequestsBlockHeaderGetErrorAction,
                PeerRemoteRequestsBlockHeaderGetFinishAction,
                PeerRemoteRequestsBlockHeaderGetInitNextAction,
                PeerRemoteRequestsBlockHeaderGetPendingAction,
                PeerRemoteRequestsBlockHeaderGetSuccessAction,
            },
            block_operations_get::{
                PeerRemoteRequestsBlockOperationsGetEnqueueAction,
                PeerRemoteRequestsBlockOperationsGetErrorAction,
                PeerRemoteRequestsBlockOperationsGetFinishAction,
                PeerRemoteRequestsBlockOperationsGetInitNextAction,
                PeerRemoteRequestsBlockOperationsGetPendingAction,
                PeerRemoteRequestsBlockOperationsGetSuccessAction,
            },
            current_branch_get::{
                PeerRemoteRequestsCurrentBranchGetFinishAction,
                PeerRemoteRequestsCurrentBranchGetInitAction,
                PeerRemoteRequestsCurrentBranchGetNextBlockErrorAction,
                PeerRemoteRequestsCurrentBranchGetNextBlockInitAction,
                PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction,
                PeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction,
                PeerRemoteRequestsCurrentBranchGetPendingAction,
                PeerRemoteRequestsCurrentBranchGetSuccessAction,
            },
        },
        PeerCurrentHeadUpdateAction, PeerTryReadLoopFinishAction, PeerTryReadLoopStartAction,
        PeerTryWriteLoopFinishAction, PeerTryWriteLoopStartAction,
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

pub static FUZZER_ARGS: Lazy<RwLock<Option<fuzzcheck::Arguments>>> =
    Lazy::new(|| RwLock::new(None));

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
    TestBootstrap(BootstrapActionTest),
    TestRemoteRequest(RemoteRequestActionTest),
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
            Self::TestBootstrap(a) => a.to_action(),
            Self::TestRemoteRequest(a) => a.to_action(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum RemoteRequestActionTest {
    TestPeerRemoteRequestsBlockHeaderGetEnqueueAction(
        PeerRemoteRequestsBlockHeaderGetEnqueueAction,
    ),
    TestPeerRemoteRequestsBlockHeaderGetInitNextAction(
        PeerRemoteRequestsBlockHeaderGetInitNextAction,
    ),
    TestPeerRemoteRequestsBlockHeaderGetPendingAction(
        PeerRemoteRequestsBlockHeaderGetPendingAction,
    ),
    TestPeerRemoteRequestsBlockHeaderGetErrorAction(PeerRemoteRequestsBlockHeaderGetErrorAction),
    TestPeerRemoteRequestsBlockHeaderGetSuccessAction(
        PeerRemoteRequestsBlockHeaderGetSuccessAction,
    ),
    TestPeerRemoteRequestsBlockHeaderGetFinishAction(PeerRemoteRequestsBlockHeaderGetFinishAction),
    TestPeerRemoteRequestsBlockOperationsGetEnqueueAction(
        PeerRemoteRequestsBlockOperationsGetEnqueueAction,
    ),
    TestPeerRemoteRequestsBlockOperationsGetInitNextAction(
        PeerRemoteRequestsBlockOperationsGetInitNextAction,
    ),
    TestPeerRemoteRequestsBlockOperationsGetPendingAction(
        PeerRemoteRequestsBlockOperationsGetPendingAction,
    ),
    TestPeerRemoteRequestsBlockOperationsGetErrorAction(
        PeerRemoteRequestsBlockOperationsGetErrorAction,
    ),
    TestPeerRemoteRequestsBlockOperationsGetSuccessAction(
        PeerRemoteRequestsBlockOperationsGetSuccessAction,
    ),
    TestPeerRemoteRequestsBlockOperationsGetFinishAction(
        PeerRemoteRequestsBlockOperationsGetFinishAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetInitAction(PeerRemoteRequestsCurrentBranchGetInitAction),
    TestPeerRemoteRequestsCurrentBranchGetPendingAction(
        PeerRemoteRequestsCurrentBranchGetPendingAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetNextBlockInitAction(
        PeerRemoteRequestsCurrentBranchGetNextBlockInitAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetNextBlockPendingAction(
        PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetNextBlockErrorAction(
        PeerRemoteRequestsCurrentBranchGetNextBlockErrorAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction(
        PeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetSuccessAction(
        PeerRemoteRequestsCurrentBranchGetSuccessAction,
    ),
    TestPeerRemoteRequestsCurrentBranchGetFinishAction(
        PeerRemoteRequestsCurrentBranchGetFinishAction,
    ),
}

impl RemoteRequestActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPeerRemoteRequestsBlockHeaderGetEnqueueAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockHeaderGetInitNextAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockHeaderGetPendingAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockHeaderGetErrorAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockHeaderGetSuccessAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockHeaderGetFinishAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetEnqueueAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetInitNextAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetPendingAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetErrorAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetSuccessAction(a) => a.into(),
            Self::TestPeerRemoteRequestsBlockOperationsGetFinishAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetInitAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetPendingAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetNextBlockInitAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetNextBlockPendingAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetNextBlockErrorAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetSuccessAction(a) => a.into(),
            Self::TestPeerRemoteRequestsCurrentBranchGetFinishAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum BootstrapActionTest {
    TestBootstrapInitAction(BootstrapInitAction),
    TestBootstrapPeersConnectPendingAction(BootstrapPeersConnectPendingAction),
    TestBootstrapPeersConnectSuccessAction(BootstrapPeersConnectSuccessAction),
    TestBootstrapPeersMainBranchFindInitAction(BootstrapPeersMainBranchFindInitAction),
    TestBootstrapPeersMainBranchFindPendingAction(BootstrapPeersMainBranchFindPendingAction),
    TestBootstrapPeerCurrentBranchReceivedAction(BootstrapPeerCurrentBranchReceivedAction),
    TestBootstrapPeersMainBranchFindSuccessAction(BootstrapPeersMainBranchFindSuccessAction),
    TestBootstrapPeersBlockHeadersGetInitAction(BootstrapPeersBlockHeadersGetInitAction),
    TestBootstrapPeersBlockHeadersGetPendingAction(BootstrapPeersBlockHeadersGetPendingAction),
    TestBootstrapPeerBlockHeaderGetInitAction(BootstrapPeerBlockHeaderGetInitAction),
    TestBootstrapPeerBlockHeaderGetPendingAction(BootstrapPeerBlockHeaderGetPendingAction),
    TestBootstrapPeerBlockHeaderGetTimeoutAction(BootstrapPeerBlockHeaderGetTimeoutAction),
    TestBootstrapPeerBlockHeaderGetSuccessAction(BootstrapPeerBlockHeaderGetSuccessAction),
    TestBootstrapPeerBlockHeaderGetFinishAction(BootstrapPeerBlockHeaderGetFinishAction),
    TestBootstrapPeersBlockHeadersGetSuccessAction(BootstrapPeersBlockHeadersGetSuccessAction),
    TestBootstrapPeersBlockOperationsGetInitAction(BootstrapPeersBlockOperationsGetInitAction),
    TestBootstrapPeersBlockOperationsGetPendingAction(
        BootstrapPeersBlockOperationsGetPendingAction,
    ),
    TestBootstrapPeersBlockOperationsGetNextAllAction(
        BootstrapPeersBlockOperationsGetNextAllAction,
    ),
    TestBootstrapPeersBlockOperationsGetNextAction(BootstrapPeersBlockOperationsGetNextAction),
    TestBootstrapPeerBlockOperationsGetPendingAction(BootstrapPeerBlockOperationsGetPendingAction),
    TestBootstrapPeerBlockOperationsGetTimeoutAction(BootstrapPeerBlockOperationsGetTimeoutAction),
    TestBootstrapPeerBlockOperationsGetRetryAction(BootstrapPeerBlockOperationsGetRetryAction),
    TestBootstrapPeerBlockOperationsReceivedAction(BootstrapPeerBlockOperationsReceivedAction),
    TestBootstrapPeerBlockOperationsGetSuccessAction(BootstrapPeerBlockOperationsGetSuccessAction),
    TestBootstrapScheduleBlocksForApplyAction(BootstrapScheduleBlocksForApplyAction),
    TestBootstrapScheduleBlockForApplyAction(BootstrapScheduleBlockForApplyAction),
    TestBootstrapPeersBlockOperationsGetSuccessAction(
        BootstrapPeersBlockOperationsGetSuccessAction,
    ),
    TestBootstrapCheckTimeoutsInitAction(BootstrapCheckTimeoutsInitAction),
    TestBootstrapErrorAction(BootstrapErrorAction),
    TestBootstrapFinishedAction(BootstrapFinishedAction),
    TestBootstrapFromPeerCurrentHeadAction(BootstrapFromPeerCurrentHeadAction),
}

impl BootstrapActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestBootstrapInitAction(a) => a.into(),
            Self::TestBootstrapPeersConnectPendingAction(a) => a.into(),
            Self::TestBootstrapPeersConnectSuccessAction(a) => a.into(),
            Self::TestBootstrapPeersMainBranchFindInitAction(a) => a.into(),
            Self::TestBootstrapPeersMainBranchFindPendingAction(a) => a.into(),
            Self::TestBootstrapPeerCurrentBranchReceivedAction(a) => a.into(),
            Self::TestBootstrapPeersMainBranchFindSuccessAction(a) => a.into(),
            Self::TestBootstrapPeersBlockHeadersGetInitAction(a) => a.into(),
            Self::TestBootstrapPeersBlockHeadersGetPendingAction(a) => a.into(),
            Self::TestBootstrapPeerBlockHeaderGetInitAction(a) => a.into(),
            Self::TestBootstrapPeerBlockHeaderGetPendingAction(a) => a.into(),
            Self::TestBootstrapPeerBlockHeaderGetTimeoutAction(a) => a.into(),
            Self::TestBootstrapPeerBlockHeaderGetSuccessAction(a) => a.into(),
            Self::TestBootstrapPeerBlockHeaderGetFinishAction(a) => a.into(),
            Self::TestBootstrapPeersBlockHeadersGetSuccessAction(a) => a.into(),
            Self::TestBootstrapPeersBlockOperationsGetInitAction(a) => a.into(),
            Self::TestBootstrapPeersBlockOperationsGetPendingAction(a) => a.into(),
            Self::TestBootstrapPeersBlockOperationsGetNextAllAction(a) => a.into(),
            Self::TestBootstrapPeersBlockOperationsGetNextAction(a) => a.into(),
            Self::TestBootstrapPeerBlockOperationsGetPendingAction(a) => a.into(),
            Self::TestBootstrapPeerBlockOperationsGetTimeoutAction(a) => a.into(),
            Self::TestBootstrapPeerBlockOperationsGetRetryAction(a) => a.into(),
            Self::TestBootstrapPeerBlockOperationsReceivedAction(a) => a.into(),
            Self::TestBootstrapPeerBlockOperationsGetSuccessAction(a) => a.into(),
            Self::TestBootstrapScheduleBlocksForApplyAction(a) => a.into(),
            Self::TestBootstrapScheduleBlockForApplyAction(a) => a.into(),
            Self::TestBootstrapPeersBlockOperationsGetSuccessAction(a) => a.into(),
            Self::TestBootstrapCheckTimeoutsInitAction(a) => a.into(),
            Self::TestBootstrapErrorAction(a) => a.into(),
            Self::TestBootstrapFinishedAction(a) => a.into(),
            Self::TestBootstrapFromPeerCurrentHeadAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum StatsActionTest {
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
    TestStatsCurrentHeadPrecheckInitAction(
        stats_current_head_actions::StatsCurrentHeadPrecheckInitAction,
    ),
    TestCurrentHeadRehydrateInitAction(CurrentHeadRehydrateInitAction),
    TestCurrentHeadRehydratePendingAction(CurrentHeadRehydratePendingAction),
    TestCurrentHeadRehydrateErrorAction(CurrentHeadRehydrateErrorAction),
    TestCurrentHeadRehydrateSuccessAction(CurrentHeadRehydrateSuccessAction),
    TestCurrentHeadRehydratedAction(CurrentHeadRehydratedAction),
    TestCurrentHeadUpdateAction(CurrentHeadUpdateAction),
}

impl StatsActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestStatsCurrentHeadPrecheckSuccessAction(a) => a.into(),
            Self::TestStatsCurrentHeadPrepareSendAction(a) => a.into(),
            Self::TestStatsCurrentHeadSentAction(a) => a.into(),
            Self::TestStatsCurrentHeadSentErrorAction(a) => a.into(),
            Self::TestStatsCurrentHeadPrecheckInitAction(a) => a.into(),
            Self::TestCurrentHeadRehydrateInitAction(a) => a.into(),
            Self::TestCurrentHeadRehydratePendingAction(a) => a.into(),
            Self::TestCurrentHeadRehydrateErrorAction(a) => a.into(),
            Self::TestCurrentHeadRehydrateSuccessAction(a) => a.into(),
            Self::TestCurrentHeadRehydratedAction(a) => a.into(),
            Self::TestCurrentHeadUpdateAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum CurrentHeadActionTest {
    TestCurrentHeadReceivedAction(current_head_precheck::CurrentHeadReceivedAction),
    TestCurrentHeadPrecheckAction(current_head_precheck::CurrentHeadPrecheckAction),
    TestCurrentHeadPrecheckSuccessAction(current_head_precheck::CurrentHeadPrecheckSuccessAction),
    TestCurrentHeadPrecheckRejectedAction(current_head_precheck::CurrentHeadPrecheckRejectedAction),
    TestCurrentHeadErrorAction(current_head_precheck::CurrentHeadErrorAction),
    TestCurrentHeadPrecacheBakingRightsAction(
        current_head_precheck::CurrentHeadPrecacheBakingRightsAction,
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
            Self::TestCurrentHeadPrecacheBakingRightsAction(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum RightsActionTest {
    TestRightsGetAction(rights_actions::RightsGetAction),
    TestRightsInitAction(rights_actions::RightsInitAction),
    TestRightsEndorsingOldReadyAction(rights_actions::RightsEndorsingOldReadyAction),
    TestRightsBakingOldReadyAction(rights_actions::RightsBakingOldReadyAction),
    TestRightsErrorAction(rights_actions::RightsErrorAction),
    TestRightsRpcGetAction(rights_actions::RightsRpcGetAction),
    TestRightsRpcEndorsingReadyAction(rights_actions::RightsRpcEndorsingReadyAction),
    TestRightsRpcBakingReadyAction(rights_actions::RightsRpcBakingReadyAction),
    TestRightsRpcErrorAction(rights_actions::RightsRpcErrorAction),
    TestRightsRpcPruneAction(rights_actions::RightsRpcPruneAction),
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
    TestRightsGetCycleDelegatesAction(rights_actions::RightsGetCycleDelegatesAction),
    TestRightsCycleDelegatesReadyAction(rights_actions::RightsCycleDelegatesReadyAction),
    TestRightsCalculateIthacaAction(rights_actions::RightsCalculateIthacaAction),
    TestRightsContextRequestedAction(rights_actions::RightsContextRequestedAction),
    TestRightsIthacaContextSuccessAction(rights_actions::RightsIthacaContextSuccessAction),
    TestRightsEndorsingReadyAction(rights_actions::RightsEndorsingReadyAction),

    TestRightsCycleDelegatesGetAction(
        rights_cycle_delegates_actions::RightsCycleDelegatesGetAction,
    ),
    TestRightsCycleDelegatesRequestedAction(
        rights_cycle_delegates_actions::RightsCycleDelegatesRequestedAction,
    ),
    TestRightsCycleDelegatesSuccessAction(
        rights_cycle_delegates_actions::RightsCycleDelegatesSuccessAction,
    ),
    TestRightsCycleDelegatesErrorAction(
        rights_cycle_delegates_actions::RightsCycleDelegatesErrorAction,
    ),

    TestRightsCycleErasGetAction(rights_cycle_eras_actions::RightsCycleErasGetAction),
    TestRightsCycleErasKVSuccessAction(rights_cycle_eras_actions::RightsCycleErasKVSuccessAction),
    TestRightsCycleErasKVErrorAction(rights_cycle_eras_actions::RightsCycleErasKVErrorAction),
    TestRightsCycleErasContextRequestedAction(
        rights_cycle_eras_actions::RightsCycleErasContextRequestedAction,
    ),
    TestRightsCycleErasContextSuccessAction(
        rights_cycle_eras_actions::RightsCycleErasContextSuccessAction,
    ),
    TestRightsCycleErasContextErrorAction(
        rights_cycle_eras_actions::RightsCycleErasContextErrorAction,
    ),
    TestRightsCycleErasSuccessAction(rights_cycle_eras_actions::RightsCycleErasSuccessAction),
    TestRightsCycleErasErrorAction(rights_cycle_eras_actions::RightsCycleErasErrorAction),
}

impl RightsActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestRightsGetAction(a) => a.into(),
            Self::TestRightsInitAction(a) => a.into(),
            Self::TestRightsEndorsingOldReadyAction(a) => a.into(),
            Self::TestRightsBakingOldReadyAction(a) => a.into(),
            Self::TestRightsErrorAction(a) => a.into(),
            Self::TestRightsRpcGetAction(a) => a.into(),
            Self::TestRightsRpcEndorsingReadyAction(a) => a.into(),
            Self::TestRightsRpcBakingReadyAction(a) => a.into(),
            Self::TestRightsRpcErrorAction(a) => a.into(),
            Self::TestRightsRpcPruneAction(a) => a.into(),
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
            Self::TestRightsGetCycleDelegatesAction(a) => a.into(),
            Self::TestRightsCycleDelegatesReadyAction(a) => a.into(),
            Self::TestRightsCalculateIthacaAction(a) => a.into(),
            Self::TestRightsContextRequestedAction(a) => a.into(),
            Self::TestRightsIthacaContextSuccessAction(a) => a.into(),
            Self::TestRightsEndorsingReadyAction(a) => a.into(),

            Self::TestRightsCycleDelegatesGetAction(a) => a.into(),
            Self::TestRightsCycleDelegatesRequestedAction(a) => a.into(),
            Self::TestRightsCycleDelegatesSuccessAction(a) => a.into(),
            Self::TestRightsCycleDelegatesErrorAction(a) => a.into(),

            Self::TestRightsCycleErasGetAction(a) => a.into(),
            Self::TestRightsCycleErasKVSuccessAction(a) => a.into(),
            Self::TestRightsCycleErasKVErrorAction(a) => a.into(),
            Self::TestRightsCycleErasContextRequestedAction(a) => a.into(),
            Self::TestRightsCycleErasContextSuccessAction(a) => a.into(),
            Self::TestRightsCycleErasContextErrorAction(a) => a.into(),
            Self::TestRightsCycleErasSuccessAction(a) => a.into(),
            Self::TestRightsCycleErasErrorAction(a) => a.into(),
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
    TestMempoolGetPendingOperationsAction(mempool_actions::MempoolGetPendingOperationsAction),
    TestMempoolOperationDecodedAction(mempool_actions::MempoolOperationDecodedAction),
    TestMempoolRpcEndorsementsStatusGetAction(
        mempool_actions::MempoolRpcEndorsementsStatusGetAction,
    ),
    TestMempoolBlockInjectAction(mempool_actions::BlockInjectAction),
    TestMempoolOperationValidateNext(mempool_actions::MempoolOperationValidateNextAction),
    TestMempoolValidatorInit(mempool_validator::MempoolValidatorInitAction),
    TestMempoolValidatorPending(mempool_validator::MempoolValidatorPendingAction),
    TestMempoolValidatorSuccess(mempool_validator::MempoolValidatorSuccessAction),
    TestMempoolValidatorReady(mempool_validator::MempoolValidatorReadyAction),
    TestMempoolValidatorValidateInit(mempool_validator::MempoolValidatorValidateInitAction),
    TestMempoolValidatorValidatePending(mempool_validator::MempoolValidatorValidatePendingAction),
    TestMempoolValidatorValidateSuccess(mempool_validator::MempoolValidatorValidateSuccessAction),
}

impl MempoolActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestMempoolRecvDoneAction(a) => a.into(),
            Self::TestMempoolGetOperationsAction(a) => a.into(),
            Self::TestMempoolMarkOperationsAsPendingAction(a) => a.into(),
            Self::TestMempoolOperationRecvDoneAction(a) => a.into(),
            Self::TestMempoolOperationInjectAction(a) => a.into(),
            Self::TestMempoolRpcRespondAction(a) => a.into(),
            Self::TestMempoolRegisterOperationsStreamAction(a) => a.into(),
            Self::TestMempoolUnregisterOperationsStreamsAction(a) => a.into(),
            Self::TestMempoolSendAction(a) => a.into(),
            Self::TestMempoolSendValidatedAction(a) => a.into(),
            Self::TestMempoolAskCurrentHeadAction(a) => a.into(),
            Self::TestMempoolBroadcastAction(a) => a.into(),
            Self::TestMempoolBroadcastDoneAction(a) => a.into(),
            Self::TestMempoolGetPendingOperationsAction(a) => a.into(),
            Self::TestMempoolOperationDecodedAction(a) => a.into(),
            Self::TestMempoolRpcEndorsementsStatusGetAction(a) => a.into(),
            Self::TestMempoolBlockInjectAction(a) => a.into(),
            Self::TestMempoolOperationValidateNext(a) => a.into(),
            Self::TestMempoolValidatorInit(a) => a.into(),
            Self::TestMempoolValidatorPending(a) => a.into(),
            Self::TestMempoolValidatorSuccess(a) => a.into(),
            Self::TestMempoolValidatorReady(a) => a.into(),
            Self::TestMempoolValidatorValidateInit(a) => a.into(),
            Self::TestMempoolValidatorValidatePending(a) => a.into(),
            Self::TestMempoolValidatorValidateSuccess(a) => a.into(),
        }
    }
}

#[derive(fuzzcheck::DefaultMutator, Serialize, Deserialize, Debug, Clone)]
enum PrecheckerActionTest {
    TestPrecheckerCurrentHeadUpdateAction(prechecker_actions::PrecheckerCurrentHeadUpdateAction),
    TestPrecheckerStoreEndorsementBranchAction(
        prechecker_actions::PrecheckerStoreEndorsementBranchAction,
    ),
    TestPrecheckerPrecheckOperationAction(prechecker_actions::PrecheckerPrecheckOperationAction),
    TestPrecheckerPrecheckDelayedOperationAction(
        prechecker_actions::PrecheckerPrecheckDelayedOperationAction,
    ),
    TestPrecheckerDecodeOperationAction(prechecker_actions::PrecheckerDecodeOperationAction),
    TestPrecheckerCategorizeOperationAction(
        prechecker_actions::PrecheckerCategorizeOperationAction,
    ),
    TestPrecheckerProtocolNeededAction(prechecker_actions::PrecheckerProtocolNeededAction),
    TestPrecheckerValidateOperationAction(prechecker_actions::PrecheckerValidateOperationAction),
    TestPrecheckerOperationValidatedAction(prechecker_actions::PrecheckerOperationValidatedAction),
    TestPrecheckerErrorAction(prechecker_actions::PrecheckerErrorAction),
    TestPrecheckerCacheProtocolAction(prechecker_actions::PrecheckerCacheProtocolAction),
    TestPrecheckerCacheDelayedOperationAction(
        prechecker_actions::PrecheckerCacheDelayedOperationAction,
    ),
    TestPrecheckerPruneOperationAction(prechecker_actions::PrecheckerPruneOperationAction),
}

impl PrecheckerActionTest {
    fn to_action(&self) -> Action {
        match self.clone() {
            Self::TestPrecheckerCurrentHeadUpdateAction(a) => a.into(),
            Self::TestPrecheckerStoreEndorsementBranchAction(a) => a.into(),
            Self::TestPrecheckerPrecheckOperationAction(a) => a.into(),
            Self::TestPrecheckerPrecheckDelayedOperationAction(a) => a.into(),
            Self::TestPrecheckerDecodeOperationAction(a) => a.into(),
            Self::TestPrecheckerCategorizeOperationAction(a) => a.into(),
            Self::TestPrecheckerProtocolNeededAction(a) => a.into(),
            Self::TestPrecheckerValidateOperationAction(a) => a.into(),
            Self::TestPrecheckerOperationValidatedAction(a) => a.into(),
            Self::TestPrecheckerErrorAction(a) => a.into(),
            Self::TestPrecheckerCacheProtocolAction(a) => a.into(),
            Self::TestPrecheckerCacheDelayedOperationAction(a) => a.into(),
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
    TestPeerCurrentHeadUpdateAction(PeerCurrentHeadUpdateAction),
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
            Self::TestPeerCurrentHeadUpdateAction(a) => a.into(),
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

fn save_state(state: &State) {
    let args = FUZZER_ARGS.read().unwrap().clone().unwrap();
    let artifacts_folder = args.artifacts_folder.unwrap();
    let mut hasher = DefaultHasher::new();
    let contents = state.encode().unwrap();

    contents.hash(&mut hasher);

    let hash = hasher.finish();
    let name = format!("{:x}", hash);
    let path = artifacts_folder.join(&name);
    println!("Saving state at {:?}", path);
    std::fs::write(path, &contents).unwrap();
}

fn reduce_with_state(action: &ActionWithMeta<Action>) -> bool {
    let prev_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        let bt = std::backtrace::Backtrace::force_capture();
        println!("NEW CRASH {}", panic_info);
        println!("{:?}", bt);
    }));

    let mut state = FUZZER_STATE.write().unwrap();

    if state.reset_count != 0 && state.iteration_count == state.reset_count {
        //println!("Resetting state: iteration {}", state.iteration_count);
        state.current_target_state = state.initial_target_state.clone();
        state.iteration_count = 0;
    }

    if state.iteration_count == 0 && state.actions.is_some() {
        let actions = state.actions.as_ref().unwrap().clone();

        if state.action_count >= actions.len() {
            state.action_count = 0;
        }

        //println!("replaying {} actions", state.action_count);

        for action in actions.iter().take(state.action_count) {
            let last_id = state.current_target_state.last_action.time_as_nanos();
            let action_meta = ActionWithMeta {
                id: ActionId::new_unchecked(last_id + 1),
                depth: 0,
                action: action.clone(),
            };

            shell_automaton::reducer(&mut state.current_target_state, &action_meta);
        }

        state.action_count += 1;
    }

    state.iteration_count += 1;

    let result = catch_unwind(AssertUnwindSafe(|| {
        if !is_action_enabled(action.action.clone(), &state.current_target_state) {
            true
        } else {
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
    }));

    std::panic::set_hook(prev_hook);

    match result {
        Ok(result) => {
            if result == false {
                save_state(&state.current_target_state);
            }

            result
        }
        Err(err) => {
            save_state(&state.current_target_state);
            std::panic::resume_unwind(err)
        }
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

fn is_action_enabled(action: Action, state: &State) -> bool {
    action.is_enabled(state)
}

pub fn handle_alloc_error(layout: std::alloc::Layout) {
    let bt = std::backtrace::Backtrace::force_capture();
    println!("Allocation error {:?}", layout.size());
    println!("{:?}", bt);
    //std::process::exit(1)
}

#[cfg(test)]
#[test]
fn test_all() {
    std::alloc::set_alloc_error_hook(handle_alloc_error);

    let builder = fuzzcheck::fuzz_test(action_test_all)
        .mutator(AllActionsTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck();

    // *FUZZER_ARGS.write().unwrap() = Some(builder.arguments.clone());
    builder.stop_after_first_test_failure(true).launch();
}
