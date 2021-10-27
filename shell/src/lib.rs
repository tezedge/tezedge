// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate contains all shell actors plus few types used to handle the complexity of chain synchronisation process.

use thiserror::Error;

pub mod chain_feeder;
pub mod chain_manager;
pub mod mempool;
pub mod peer_branch_bootstrapper;
pub mod shell_automaton_manager;
pub mod shell_channel;
pub mod state;
pub mod stats;
pub mod validation;

pub use shell_automaton::shell_compatibility_version::ShellCompatibilityVersion;

/// Constant tells about p2p feature versions, which this shell is compatible with
pub const SUPPORTED_P2P_VERSION: &[u16] = &[0, 1];

/// Constant tells about distributed_db feature versions, which this shell is compatible with
pub const SUPPORTED_DISTRIBUTED_DB_VERSION: &[u16] = &[0];

#[derive(Debug, Clone, Error)]
#[error("InvalidRange - {0}")]
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

pub mod subscription {
    use tezedge_actor_system::actors::*;

    use networking::network_channel::NetworkChannelTopic;

    #[inline]
    pub fn subscribe_to_actor_terminated<M, E>(sys_channel: &ChannelRef<E>, myself: ActorRef<M>)
    where
        M: Message,
        E: Message + Into<M>,
    {
        sys_channel.tell(
            Subscribe {
                topic: SysTopic::ActorTerminated.into(),
                actor: Box::new(myself),
            },
            None,
        );
    }

    #[inline]
    pub fn subscribe_to_network_events<M, E>(network_channel: &ChannelRef<E>, myself: ActorRef<M>)
    where
        M: Message,
        E: Message + Into<M>,
    {
        network_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: NetworkChannelTopic::NetworkEvents.into(),
            },
            None,
        );
    }

    #[inline]
    pub fn subscribe_with_response_to_network_events<M, E>(
        network_channel: &ChannelRef<E>,
        myself: ActorRef<M>,
        response: M,
    ) where
        M: Message,
        E: Message + Into<M>,
    {
        network_channel.tell(
            SubscribeWithResponse {
                actor: Box::new(myself.clone()),
                topic: NetworkChannelTopic::NetworkEvents.into(),
                response: AnyMessage::new(response, true),
            },
            Some(myself.into()),
        );
    }

    #[inline]
    pub fn subscribe_to_shell_events<M, E>(shell_channel: &ChannelRef<E>, myself: ActorRef<M>)
    where
        M: Message,
        E: Message + Into<M>,
    {
        shell_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: crate::shell_channel::ShellChannelTopic::ShellEvents.into(),
            },
            None,
        );
    }

    #[inline]
    pub fn subscribe_to_shell_new_current_head<M, E>(
        shell_channel: &ChannelRef<E>,
        myself: ActorRef<M>,
    ) where
        M: Message,
        E: Message + Into<M>,
    {
        shell_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: crate::shell_channel::ShellChannelTopic::ShellNewCurrentHead.into(),
            },
            None,
        );
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

/// Module implements shell integration based on actual shell constalation.
/// In case of new architecture (state machine, ..., whatever),
/// we just need to reimplement [ShellConnectorSupport] here and replace shell_channel with ShellAutomatonSender or whatever
pub mod connector {
    use std::sync::Arc;

    use crypto::hash::ChainId;
    use shell_integration::*;

    use crate::chain_manager::{AskPeersAboutCurrentHead, ChainManagerRef, InjectBlockRequest};
    use crate::mempool::MempoolPrevalidatorFactory;

    pub struct ShellConnectorSupport {
        chain_manager: ChainManagerRef,
        mempool_prevalidator_factory: Arc<MempoolPrevalidatorFactory>,
    }

    impl ShellConnectorSupport {
        pub fn new(
            chain_manager: ChainManagerRef,
            mempool_prevalidator_factory: Arc<MempoolPrevalidatorFactory>,
        ) -> Self {
            ShellConnectorSupport {
                chain_manager,
                mempool_prevalidator_factory,
            }
        }
    }

    impl ShellConnector for ShellConnectorSupport {
        fn request_current_head_from_connected_peers(&self) {
            // actual implementation use tezedge_actor_system and sends command to shell_channel
            use tezedge_actor_system::actors::*;

            self.chain_manager.tell(
                AskPeersAboutCurrentHead {
                    last_received_timeout: None,
                },
                None,
            );
        }

        fn find_mempool_prevalidators(&self) -> Result<Vec<Prevalidator>, UnexpectedError> {
            self.mempool_prevalidator_factory
                .find_mempool_prevalidators()
        }

        fn find_mempool_prevalidator_caller(
            &self,
            chain_id: &ChainId,
        ) -> Option<Box<dyn MempoolPrevalidatorCaller>> {
            self.mempool_prevalidator_factory
                .find_mempool_prevalidator_caller(chain_id)
                .map(|c| Box::new(c) as Box<dyn MempoolPrevalidatorCaller>)
        }
    }

    impl InjectBlockConnector for ShellConnectorSupport {
        fn inject_block(
            &self,
            request: InjectBlock,
            result_callback: Option<InjectBlockOneshotResultCallback>,
        ) {
            // actual implementation use tezedge_actor_system and sends command to shell_channel
            use tezedge_actor_system::actors::*;

            self.chain_manager.tell(
                InjectBlockRequest {
                    request,
                    result_callback,
                },
                None,
            );
        }
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
