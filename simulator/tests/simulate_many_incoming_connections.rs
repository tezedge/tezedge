use std::time::{Duration, Instant};

use simulator::one_real_node_cluster::*;
use tezedge_state::proposer::TezedgeProposerConfig;
use tezedge_state::{sample_tezedge_state, TezedgeConfig, TezedgeState};

fn default_state(initial_time: Instant) -> TezedgeState {
    sample_tezedge_state::build(
        initial_time,
        TezedgeConfig {
            port: 9732,
            disable_mempool: false,
            private_node: false,
            disable_quotas: false,
            disable_blacklist: false,
            min_connected_peers: 500,
            max_connected_peers: 1000,
            max_pending_peers: 900,
            max_potential_peers: 1000,
            periodic_react_interval: Duration::from_millis(250),
            reset_quotas_interval: Duration::from_secs(5),
            peer_blacklist_duration: Duration::from_secs(15 * 60),
            peer_timeout: Duration::from_secs(8),
            pow_target: 0.0,
        },
    )
}

fn default_cluster() -> OneRealNodeCluster {
    let initial_time = Instant::now();
    OneRealNodeCluster::new(
        initial_time,
        TezedgeProposerConfig {
            wait_for_events_timeout: Some(Duration::from_millis(250)),
            events_limit: 1024,
        },
        default_state(initial_time),
    )
}

#[test]
fn simulate_many_incoming_connections() {
    let mut cluster = default_cluster();

    for _ in 0..10000 {
        let peer_id = cluster.init_new_fake_peer();
        if let Ok(_) = cluster.connect_to_node(peer_id) {
            cluster.add_writable_event(peer_id, None);
        }
        cluster
            .advance_time_ms(5)
            .make_progress()
            .assert_state();
    }

    cluster.add_tick_for_current_time();

    while !cluster.is_done() {
        cluster.make_progress();
        cluster.assert_state();
    }
}
