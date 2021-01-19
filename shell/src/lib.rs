// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate contains all shell actors plus few types used to handle the complexity of chain synchronisation process.

mod state;

pub mod chain_feeder;
pub mod chain_manager;
pub mod context_listener;
pub mod mempool;
pub mod peer_manager;
pub mod shell_channel;
pub mod stats;
pub mod utils;
pub mod validation;

/// Constant tells about p2p feature versions, which this shell is compatible with
pub const SUPPORTED_P2P_VERSION: &[u16] = &[0, 1];

/// Constant tells about distributed_db feature versions, which this shell is compatible with
pub const SUPPORTED_DISTRIBUTED_DB_VERSION: &[u16] = &[0];

/// Simple threshold, for representing integral ranges.
#[derive(Copy, Clone, Debug)]
pub struct PeerConnectionThreshold {
    low: usize,
    high: usize,
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
    pub fn new(low: usize, high: usize, peers_for_bootstrap_threshold: Option<usize>) -> Self {
        assert!(low <= high, "low must be less than or equal to high");
        PeerConnectionThreshold {
            low,
            high,
            peers_for_bootstrap_threshold,
        }
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
    use riker::actors::*;

    use networking::p2p::network_channel::NetworkChannelTopic;

    use crate::shell_channel::ShellChannelTopic;

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
    pub(crate) fn subscribe_to_network_commands<M, E>(
        network_channel: &ChannelRef<E>,
        myself: ActorRef<M>,
    ) where
        M: Message,
        E: Message + Into<M>,
    {
        network_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: NetworkChannelTopic::NetworkCommands.into(),
            },
            None,
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
                actor: Box::new(myself.clone()),
                topic: ShellChannelTopic::ShellEvents.into(),
            },
            None,
        );
    }

    #[inline]
    pub(crate) fn subscribe_to_shell_commands<M, E>(
        shell_channel: &ChannelRef<E>,
        myself: ActorRef<M>,
    ) where
        M: Message,
        E: Message + Into<M>,
    {
        shell_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: ShellChannelTopic::ShellCommands.into(),
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
                actor: Box::new(myself.clone()),
                topic: ShellChannelTopic::ShellShutdown.into(),
            },
            None,
        );
    }

    #[inline]
    pub(crate) fn subscribe_to_dead_letters<M, E>(dl_channel: &ChannelRef<E>, myself: ActorRef<M>)
    where
        M: Message,
        E: Message + Into<M>,
    {
        dl_channel.tell(
            Subscribe {
                actor: Box::new(myself),
                topic: All.into(),
            },
            None,
        );
    }

    #[inline]
    pub(crate) fn unsubscribe_from_dead_letters<M, E>(
        dl_channel: &ChannelRef<E>,
        myself: ActorRef<M>,
    ) where
        M: Message,
        E: Message + Into<M>,
    {
        dl_channel.tell(
            Unsubscribe {
                actor: Box::new(myself),
                topic: All.into(),
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
        let peer_threshold = PeerConnectionThreshold::new(0, 10, Some(5));
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 5);

        // clasic scenario
        let peer_threshold = PeerConnectionThreshold::new(5, 500, None);
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 83);

        // calculated threshold too low (0), use minimal value of 1
        let peer_threshold = PeerConnectionThreshold::new(1, 4, None);
        let sync_threshold = peer_threshold.num_of_peers_for_bootstrap_threshold();
        assert_eq!(sync_threshold, 1);
    }
}
