// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate contains all shell actors plus few types used to handle the complexity of chain synchronisation process.

use failure::Fail;
use serde::Serialize;

pub mod chain_current_head_manager;
pub mod chain_feeder;
pub mod chain_manager;
pub mod mempool;
pub mod peer_branch_bootstrapper;
pub mod shell_channel;
pub mod state;
pub mod stats;
pub mod tezedge_state_manager;
pub mod utils;
pub mod validation;

/// Constant tells about p2p feature versions, which this shell is compatible with
pub const SUPPORTED_P2P_VERSION: &[u16] = &[0, 1];

/// Constant tells about distributed_db feature versions, which this shell is compatible with
pub const SUPPORTED_DISTRIBUTED_DB_VERSION: &[u16] = &[0];

#[derive(Debug, Clone, Fail)]
#[fail(display = "InvalidRange - {}", _0)]
pub struct InvalidRangeError(String);

/// Simple threshold, for representing integral ranges.
#[derive(Copy, Clone, Debug)]
pub struct PeerConnectionThreshold {
    pub low: usize,
    pub high: usize,
    peers_for_bootstrap_threshold: Option<usize>,
}

impl PeerConnectionThreshold {
    /// Create new threshold, by specifying mnimum and maximum (inclusively).
    ///
    /// # Arguments
    /// * `low` - Lower threshold bound
    /// * `higher` - Upper threshold bound
    ///
    /// `low` cannot be bigger than `high`, otherwise function will panic
    pub fn try_new(
        low: usize,
        high: usize,
        peers_for_bootstrap_threshold: Option<usize>,
    ) -> Result<Self, InvalidRangeError> {
        if low > high {
            return Err(InvalidRangeError(format!(
                "low: {} must be less than or equal to high: {}",
                low, high
            )));
        }

        Ok(PeerConnectionThreshold {
            low,
            high,
            peers_for_bootstrap_threshold,
        })
    }

    /// Threshold for minimal count of bootstrapped peers
    /// Ocaml counts it from (expected)connections: see [node_shared_arg.ml]
    pub fn num_of_peers_for_bootstrap_threshold(&self) -> usize {
        if let Some(sync_tresh) = self.peers_for_bootstrap_threshold {
            // set a concrete value if provided in configuration
            // NOTE: in a sandbox enviroment, it's ok to set to 0
            sync_tresh
        } else {
            // calculate othervise
            // TODO TE-244 - Implement the synchronization heuristic
            // NOTE: the calculation should never yield 0!

            // since we define the low and high bound, calculate the expected connections
            // see [node_shared_arg.ml]
            let expected_connections = 2 * self.high / 3;

            // never yield 0!
            std::cmp::max(1, expected_connections / 4)
        }
    }
}

#[derive(Serialize, Debug)]
pub struct WorkerStatus {
    phase: WorkerStatusPhase,
    since: String,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum WorkerStatusPhase {
    #[serde(rename = "running")]
    Running,
}

pub mod subscription {

    use std::sync::Arc;

    use riker::actors::*;
    use slog::{warn, Logger};

    use crypto::hash::{BlockHash, ChainId};
    use networking::{PeerAddress, PeerId};
    use storage::BlockHeaderWithHash;
    use tezos_messages::p2p::encoding::metadata::MetadataMessage;
    use tezos_messages::p2p::encoding::prelude::Mempool;
    use tezos_messages::p2p::encoding::version::NetworkVersion;

    use crate::state::synchronization_state::PeerBranchSynchronizationDone;
    use crate::tezedge_state_manager::proposer_messages::*;

    pub trait Notification: std::fmt::Debug + Clone {}

    #[derive(Debug)]
    pub struct SendNotificationError {
        pub reason: String,
    }

    #[derive(Debug)]
    pub struct ShutdownCallbackError {
        pub reason: String,
    }

    pub trait NotifyCallback<N: Notification>: Sync + Send {
        fn notify(&self, notification: N) -> Result<(), SendNotificationError>;

        fn shutdown(&self) -> Result<(), ShutdownCallbackError>;
    }

    pub trait NotifyCallbackRegistrar<N: Notification> {
        fn subscribe(&self, notifier: &mut Notifier<N>);
    }

    pub struct Notifier<N: Notification> {
        subscribers: Vec<(Box<dyn NotifyCallback<N>>, String)>,
        log: Logger,
    }

    impl<N: Notification> Notifier<N> {
        pub fn new(log: Logger) -> Self {
            Self {
                subscribers: Vec::new(),
                log,
            }
        }

        pub fn register(
            &mut self,
            subscriber: Box<dyn NotifyCallback<N>>,
            subscriber_name: String,
        ) {
            self.subscribers.push((subscriber, subscriber_name))
        }

        pub fn notify(&self, notification: N) {
            for (subscriber, subscriber_name) in self.subscribers.iter() {
                if let Err(e) = subscriber.notify(notification.clone()) {
                    warn!(self.log, "Failed to notify subscriber with notification";
                        "subscriber" => subscriber_name,
                        "notification" => format!("{:?}", notification),
                        "reason" => format!("{:?}", e),
                    );
                }
            }
        }
    }

    impl<N: Notification> Drop for Notifier<N> {
        fn drop(&mut self) {
            for (subscriber, subscriber_name) in self.subscribers.iter() {
                if let Err(e) = subscriber.shutdown() {
                    warn!(self.log, "Failed to shutdown subscriber";
                        "subscriber" => subscriber_name,
                        "reason" => format!("{:?}", e));
                }
            }
        }
    }

    pub type NewCurrentHeadNotificationRef = Arc<NewCurrentHeadNotification>;

    impl Notification for NewCurrentHeadNotificationRef {}

    #[derive(Debug)]
    pub struct NewCurrentHeadNotification {
        pub chain_id: Arc<ChainId>,
        pub block: Arc<BlockHeaderWithHash>,
        pub is_bootstrapped: bool,
    }

    impl NewCurrentHeadNotification {
        pub fn new(
            chain_id: Arc<ChainId>,
            block: Arc<BlockHeaderWithHash>,
            is_bootstrapped: bool,
        ) -> Self {
            Self {
                chain_id,
                block,
                is_bootstrapped,
            }
        }
    }

    pub type NetworkEventNotificationRef = Arc<NetworkEventNotification>;

    impl Notification for NetworkEventNotificationRef {}

    #[derive(Clone, Debug)]
    pub enum NetworkEventNotification {
        PeerBootstrapped(Arc<PeerId>, MetadataMessage, Arc<NetworkVersion>),
        PeerDisconnected(PeerAddress),
        PeerBlacklisted(PeerAddress),
        PeerMessageReceived(PeerMessageReceived),
    }

    impl From<PeerMessageReceived> for NetworkEventNotificationRef {
        fn from(msg: PeerMessageReceived) -> Self {
            Arc::new(NetworkEventNotification::PeerMessageReceived(msg))
        }
    }

    pub type ShellEventNotificationRef = Arc<ShellEventNotification>;

    impl Notification for ShellEventNotificationRef {}

    #[derive(Clone, Debug)]
    pub enum ShellEventNotification {
        BlockReceived(BlockReceived),
        AllBlockOperationsReceived(AllBlockOperationsReceived),
    }

    /// Message informing actors about receiving block header
    #[derive(Clone, Debug)]
    pub struct BlockReceived {
        pub hash: BlockHash,
        pub level: i32,
    }

    /// Message informing actors about receiving all operations for a specific block
    #[derive(Clone, Debug)]
    pub struct AllBlockOperationsReceived {
        pub level: i32,
    }

    impl From<AllBlockOperationsReceived> for ShellEventNotificationRef {
        fn from(msg: AllBlockOperationsReceived) -> Self {
            Arc::new(ShellEventNotification::AllBlockOperationsReceived(msg))
        }
    }

    impl From<BlockReceived> for ShellEventNotificationRef {
        fn from(msg: BlockReceived) -> Self {
            Arc::new(ShellEventNotification::BlockReceived(msg))
        }
    }

    // TODO: this is temporary replacement for shell_channel, once everything is refactored to state machine, this can go out
    // and everything will be handled inside state machine and proposer will not need to trigger this notification outside
    pub type ShellCommandNotificationRef = ShellCommandNotification;

    impl Notification for ShellCommandNotificationRef {}

    #[derive(Clone, Debug)]
    pub enum ShellCommandNotification {
        InjectBlock(InjectBlock, Option<InjectBlockOneshotResultCallback>),
        RequestCurrentHead,
        PeerBranchSynchronizationDone(PeerBranchSynchronizationDone),
        AdvertiseToP2pNewCurrentBranch(Arc<ChainId>, Arc<BlockHash>),
        AdvertiseToP2pNewCurrentHead(Arc<ChainId>, Arc<BlockHash>),
        AdvertiseToP2pNewMempool(Arc<ChainId>, Arc<BlockHash>, Arc<Mempool>),
    }

    #[inline]
    pub fn subscribe_to_shell_shutdown<M, E>(shell_channel: &ChannelRef<E>, myself: ActorRef<M>)
    where
        M: Message,
        E: Message + Into<M>,
    {
        shell_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: crate::shell_channel::ShellChannelTopic::ShellShutdown.into(),
            },
            None,
        );
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_num_of_peers_for_bootstrap_threshold() {
        // fixed threshold
        let peer_threshold =
            PeerConnectionThreshold::try_new(0, 10, Some(5)).expect("Invalid range");
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 5);

        // clasic scenario
        let peer_threshold = PeerConnectionThreshold::try_new(5, 500, None).expect("Invalid range");
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 83);

        // calculated threshold too low (0), use minimal value of 1
        let peer_threshold = PeerConnectionThreshold::try_new(1, 4, None).expect("Invalid range");
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 1);
    }

    #[test]
    fn test_invalid_range_threshold() {
        assert!(PeerConnectionThreshold::try_new(9, 10, Some(5)).is_ok());
        assert!(PeerConnectionThreshold::try_new(10, 10, Some(5)).is_ok());
        assert!(PeerConnectionThreshold::try_new(11, 10, Some(5)).is_err());
        assert!(PeerConnectionThreshold::try_new(12, 10, Some(5)).is_err());
    }
}
