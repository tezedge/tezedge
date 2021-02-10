// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module covers functionality to resolve if node is bootstrapped
// TODO: TE-244 - reimplement

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};

use crypto::hash::CryptoboxPublicKeyHash;
use networking::PeerId;
use tezos_messages::p2p::encoding::block_header::Level;

/// Type hold information if node is bootstrapped shareable between threads/actors
/// Indicates that node/shell is bootstrapped, which means, that can broadcast stuff (new branch, new head) to the network
type BootstrappedStatusRef = Arc<AtomicBool>;

/// Trait for struct witch has updatable flag [`is_bootstrapped`]
pub trait UpdateIsBootstrapped {
    fn set_is_bootstrapped(&mut self, new_status: bool);
    fn is_bootstrapped(&self) -> bool;
}

#[derive(Clone, Debug)]
pub struct PeerBranchSynchronizationDone {
    peer: Arc<PeerId>,
    to_level: Arc<Level>,
}

impl PeerBranchSynchronizationDone {
    pub fn new(peer: Arc<PeerId>, to_level: Arc<Level>) -> Self {
        Self { peer, to_level }
    }

    pub fn peer(&self) -> &Arc<PeerId> {
        &self.peer
    }

    fn to_level(&self) -> &Arc<Level> {
        &self.to_level
    }
}

pub type SynchronizationBootstrapStateRef = Arc<RwLock<SynchronizationBootstrapState>>;

/// Inits empty mempool state storage
pub fn init_synchronization_bootstrap_state_storage(
    num_of_peers_for_bootstrap_threshold: usize,
) -> SynchronizationBootstrapStateRef {
    Arc::new(RwLock::new(SynchronizationBootstrapState::new(
        num_of_peers_for_bootstrap_threshold,
        BootstrappedStatusRef::new(AtomicBool::new(false)),
    )))
}

/// Manages bootstrap status based on number of bootstrapped peers
pub struct SynchronizationBootstrapState {
    /// Indicates threshold for minimal count of bootstrapped peers to mark chain_manager as bootstrapped
    num_of_peers_for_bootstrap_threshold: usize,

    /// Holds bootstrapped state
    current_bootstrapped_status: BootstrappedStatusRef,

    /// holder of bootstrapped peers with they highest level
    state: HashMap<CryptoboxPublicKeyHash, Level>,
}

impl SynchronizationBootstrapState {
    /// This constant is used for solving, if something is bootstrapped
    /// This means, that the compared level, should be on at least 96%
    const HIGH_LEVEL_MARGIN_PERCENTAGE: i32 = 98;

    pub fn new(
        num_of_peers_for_bootstrap_threshold: usize,
        current_bootstrapped_status: BootstrappedStatusRef,
    ) -> Self {
        // if no limit, just mark as bootstrapped
        if num_of_peers_for_bootstrap_threshold == 0 {
            current_bootstrapped_status.store(true, Ordering::Release);
        }

        Self {
            num_of_peers_for_bootstrap_threshold,
            current_bootstrapped_status,
            state: HashMap::default(),
        }
    }

    pub fn is_bootstrapped(&self) -> bool {
        self.current_bootstrapped_status.load(Ordering::Acquire)
    }

    fn consider_as_bootstrapped(
        tested_level: Level,
        target_level: Level,
        percentage_margin: i32,
    ) -> bool {
        // only tested level is higher/equals percentage of target_level then HIGH_LEVEL_MARGIN_PERCENTAGE
        if let Some(calculated_percentage) = tested_level
            .checked_mul(100)
            .map(|p| p.checked_div(target_level).unwrap_or(0))
        {
            if calculated_percentage >= percentage_margin {
                return true;
            }
        }

        false
    }

    pub(crate) fn update_by_peer_state(
        &mut self,
        new_update: &PeerBranchSynchronizationDone,
        target: &mut impl UpdateIsBootstrapped,
        remote_best_known_level: Level,
        local_best_known_level: Level,
    ) {
        // if already bootstrapped, do nothing
        if self.is_bootstrapped() {
            return;
        }

        // peer key for hashmap
        let peer_key = &new_update.peer.peer_public_key_hash;

        // lets update peer by public key hash
        if let Some(peer_to_level) = self.state.get_mut(peer_key) {
            let update_to_level = new_update.to_level().as_ref();
            if *peer_to_level < *update_to_level {
                *peer_to_level = *update_to_level;
                target.set_is_bootstrapped(Self::consider_as_bootstrapped(
                    *update_to_level,
                    remote_best_known_level,
                    Self::HIGH_LEVEL_MARGIN_PERCENTAGE,
                ));
            } else {
                target.set_is_bootstrapped(Self::consider_as_bootstrapped(
                    *peer_to_level,
                    remote_best_known_level,
                    Self::HIGH_LEVEL_MARGIN_PERCENTAGE,
                ));
            }
        } else {
            let update_to_level = new_update.to_level().as_ref();
            self.state.insert(peer_key.clone(), *update_to_level);
            target.set_is_bootstrapped(Self::consider_as_bootstrapped(
                *update_to_level,
                remote_best_known_level,
                Self::HIGH_LEVEL_MARGIN_PERCENTAGE,
            ));
        };

        let _ = self.update_by_new_local_head(remote_best_known_level, local_best_known_level);
    }

    pub(crate) fn update_by_new_local_head(
        &mut self,
        remote_best_known_level: Level,
        local_best_known_level: Level,
    ) -> bool {
        // let resolve number of bootstrapped peer matching
        let num_of_bootstrapped_peers = self.num_of_bootstrapped_peers(remote_best_known_level);

        // global bootstrap status is considered as bootstrapped, only if number of bootstrapped peers is under threshold,
        // and only if local level is considered as bootstrapped agains remove level
        // so we can mark chain_manager as bootstrapped
        if self.num_of_peers_for_bootstrap_threshold <= num_of_bootstrapped_peers {
            if Self::consider_as_bootstrapped(
                local_best_known_level,
                remote_best_known_level,
                Self::HIGH_LEVEL_MARGIN_PERCENTAGE,
            ) {
                self.current_bootstrapped_status
                    .store(true, Ordering::Release);
                self.state.clear();
            }
        }

        self.is_bootstrapped()
    }

    pub fn num_of_bootstrapped_peers(&self, best_known_level: Level) -> usize {
        self.state
            .values()
            .filter(|&peer_to_level| {
                Self::consider_as_bootstrapped(
                    *peer_to_level,
                    best_known_level,
                    Self::HIGH_LEVEL_MARGIN_PERCENTAGE,
                )
            })
            .count()
    }

    pub fn num_of_peers_for_bootstrap_threshold(&self) -> usize {
        self.num_of_peers_for_bootstrap_threshold
    }
}

#[cfg(test)]
pub mod tests {
    use std::net::SocketAddr;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    use futures::lock::Mutex as TokioMutex;
    use riker::actors::*;
    use slog::{Drain, Level, Logger};

    use crypto::hash::CryptoboxPublicKeyHash;
    use networking::p2p::network_channel::NetworkChannelRef;
    use networking::p2p::peer::Peer;
    use networking::p2p::{network_channel::NetworkChannel, peer::BootstrapOutput};
    use networking::PeerId;
    use tezos_identity::Identity;
    use tezos_messages::p2p::encoding::prelude::{MetadataMessage, NetworkVersion};

    use crate::state::peer_state::PeerState;

    use super::*;

    #[test]
    fn test_resolve_is_bootstrapped_no_threshold() {
        // prepare empty states
        let bootstrap_status = BootstrappedStatusRef::new(AtomicBool::new(false));
        let bootstrap_state = SynchronizationBootstrapState::new(0, bootstrap_status);

        // check
        assert!(bootstrap_state.is_bootstrapped());
    }

    #[test]
    fn test_resolve_is_bootstrapped() {
        // prerequizities
        let log = create_logger(Level::Debug);
        let tokio_runtime = create_tokio_runtime();
        let actor_system = SystemBuilder::new()
            .name("test_actors_apply_blocks_and_check_context")
            .log(log.clone())
            .create()
            .expect("Failed to create actor system");
        let network_channel =
            NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
        let mut peer_state1 = peer(&actor_system, network_channel.clone(), &tokio_runtime);
        let mut peer_state2 = peer(&actor_system, network_channel, &tokio_runtime);

        let done_peer = |to_level, peer_state: &PeerState| -> PeerBranchSynchronizationDone {
            PeerBranchSynchronizationDone::new(peer_state.peer_id.clone(), Arc::new(to_level))
        };

        // prepare empty states with threshold = 2
        let bootstrap_status = BootstrappedStatusRef::new(AtomicBool::new(false));
        let mut bootstrap_state = SynchronizationBootstrapState::new(2, bootstrap_status);

        // check
        assert!(!bootstrap_state.is_bootstrapped());
        assert_eq!(0, bootstrap_state.num_of_bootstrapped_peers(100));

        // update with low level
        bootstrap_state.update_by_peer_state(
            &done_peer(10, &peer_state1),
            &mut peer_state1,
            100,
            0,
        );
        bootstrap_state.update_by_peer_state(
            &done_peer(15, &peer_state2),
            &mut peer_state2,
            100,
            0,
        );
        assert_eq!(0, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(!peer_state1.is_bootstrapped());
        assert!(!peer_state2.is_bootstrapped());
        assert!(!bootstrap_state.is_bootstrapped());

        // update with one with higher level
        bootstrap_state.update_by_peer_state(
            &done_peer(98, &peer_state1),
            &mut peer_state1,
            100,
            0,
        );
        bootstrap_state.update_by_peer_state(
            &done_peer(15, &peer_state2),
            &mut peer_state2,
            100,
            0,
        );
        assert_eq!(1, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(peer_state1.is_bootstrapped());
        assert!(!peer_state2.is_bootstrapped());
        assert!(!bootstrap_state.is_bootstrapped());

        // update the same one with one more higher level
        bootstrap_state.update_by_peer_state(
            &done_peer(99, &peer_state1),
            &mut peer_state1,
            100,
            0,
        );
        bootstrap_state.update_by_peer_state(
            &done_peer(15, &peer_state2),
            &mut peer_state2,
            100,
            0,
        );
        assert_eq!(1, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(peer_state1.is_bootstrapped());
        assert!(!peer_state2.is_bootstrapped());
        assert!(!bootstrap_state.is_bootstrapped());

        // update the other with higher (threshold is ok, but not local level)
        bootstrap_state.update_by_peer_state(
            &done_peer(99, &peer_state1),
            &mut peer_state1,
            100,
            97,
        );
        bootstrap_state.update_by_peer_state(
            &done_peer(98, &peer_state2),
            &mut peer_state2,
            100,
            97,
        );
        assert_eq!(2, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(peer_state1.is_bootstrapped());
        assert!(peer_state2.is_bootstrapped());
        assert!(!bootstrap_state.is_bootstrapped());

        // update by new local head
        assert!(!bootstrap_state.update_by_new_local_head(100, 97));
        assert_eq!(2, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(peer_state1.is_bootstrapped());
        assert!(peer_state2.is_bootstrapped());
        assert!(!bootstrap_state.is_bootstrapped());

        assert!(bootstrap_state.update_by_new_local_head(100, 98));
        assert_eq!(0, bootstrap_state.num_of_bootstrapped_peers(100));
        assert!(peer_state1.is_bootstrapped());
        assert!(peer_state2.is_bootstrapped());
        assert!(bootstrap_state.is_bootstrapped());

        // shutdown actor system
        let _ = tokio_runtime.block_on(async move {
            tokio::time::timeout(Duration::from_secs(2), actor_system.shutdown()).await
        });
    }

    #[test]
    fn test_consider_as_bootstrapped() {
        assert!(SynchronizationBootstrapState::consider_as_bootstrapped(
            0, 0, 0,
        ));
        assert!(!SynchronizationBootstrapState::consider_as_bootstrapped(
            0, 1, 1,
        ));
        assert!(!SynchronizationBootstrapState::consider_as_bootstrapped(
            1, 0, 1,
        ));
        assert!(!SynchronizationBootstrapState::consider_as_bootstrapped(
            97, 100, 98,
        ));
        assert!(SynchronizationBootstrapState::consider_as_bootstrapped(
            98, 100, 98,
        ));
        assert!(SynchronizationBootstrapState::consider_as_bootstrapped(
            99, 100, 98,
        ));
        assert!(SynchronizationBootstrapState::consider_as_bootstrapped(
            100, 100, 98,
        ));
    }

    fn create_logger(level: Level) -> Logger {
        let drain = slog_async::Async::new(
            slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                .build()
                .fuse(),
        )
        .build()
        .filter_level(level)
        .fuse();

        Logger::root(drain, slog::o!())
    }

    fn create_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    }

    fn peer(
        sys: &impl ActorRefFactory,
        network_channel: NetworkChannelRef,
        tokio_runtime: &tokio::runtime::Runtime,
    ) -> PeerState {
        let socket_address: SocketAddr = "127.0.0.1:3011"
            .parse()
            .expect("Expected valid ip:port address");

        let node_identity = Arc::new(Identity::generate(0f64));
        let peer_public_key_hash: CryptoboxPublicKeyHash =
            node_identity.public_key.public_key_hash();
        let peer_id_marker = peer_public_key_hash.to_base58_check();

        let metadata = MetadataMessage::new(false, false);
        let version = NetworkVersion::new("".to_owned(), 0, 0);
        let peer_ref = Peer::actor(
            sys,
            network_channel,
            tokio_runtime.handle().clone(),
            BootstrapOutput(
                Arc::new(TokioMutex::new(None)),
                Arc::new(TokioMutex::new(None)),
                peer_public_key_hash.clone(),
                peer_id_marker.clone(),
                metadata.clone(),
                version,
                socket_address,
            ),
        )
        .unwrap();

        PeerState::new(
            Arc::new(PeerId::new(
                peer_ref,
                peer_public_key_hash,
                peer_id_marker,
                socket_address,
            )),
            &metadata,
        )
    }
}
