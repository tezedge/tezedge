use std::sync::Arc;
use std::time::{Duration, Instant};

use simulator::one_real_node_cluster::*;
use tezedge_state::proposer::TezedgeProposerConfig;
use tezedge_state::{sample_tezedge_state, DefaultEffects, TezedgeConfig, TezedgeState};

pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}

fn default_state(initial_time: Instant) -> TezedgeState {
    sample_tezedge_state::build(
        initial_time,
        TezedgeConfig {
            port: 9732,
            disable_mempool: false,
            private_node: false,
            disable_quotas: false,
            disable_blacklist: false,
            min_connected_peers: 2,
            max_connected_peers: 3,
            max_pending_peers: 3,
            max_potential_peers: 10,
            periodic_react_interval: Duration::from_millis(250),
            reset_quotas_interval: Duration::from_secs(5),
            peer_blacklist_duration: Duration::from_secs(15 * 60),
            peer_timeout: Duration::from_secs(8),
            pow_target: 1.0,
        },
        &mut DefaultEffects::default(),
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
fn test_no_infinite_loop_when_write_returns_0() {
    let control = Arc::new(());
    let is_running = Arc::downgrade(&control);

    std::thread::spawn(move || {
        let mut cluster = default_cluster();
        let peer_id = cluster.init_new_fake_peer();

        cluster
            .connect_from_node(peer_id)
            .make_progress()
            .add_writable_event(peer_id, Some(0))
            .add_tick_for_current_time();

        while !cluster.is_done() {
            cluster.make_progress();
            cluster.assert_state();
        }

        black_box(control);
    });

    std::thread::sleep(Duration::from_millis(500));
    if is_running.upgrade().is_some() {
        panic!("Thread still running. Maybe infinite loop?");
    }
}

#[test]
fn test_no_infinite_loop_when_read_returns_0() {
    let control = Arc::new(());
    let is_running = Arc::downgrade(&control);

    std::thread::spawn(move || {
        let mut cluster = default_cluster();
        let peer_id = cluster.init_new_fake_peer();

        cluster
            .connect_from_node(peer_id)
            .make_progress()
            .add_readable_event(peer_id, Some(0))
            .add_tick_for_current_time();

        while !cluster.is_done() {
            cluster.make_progress();
            cluster.assert_state();
        }

        black_box(control);
    });

    std::thread::sleep(Duration::from_millis(500));
    if is_running.upgrade().is_some() {
        panic!("Thread still running. Maybe infinite loop?");
    }
}
