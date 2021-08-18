// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// use std::time::{Duration, Instant};

// use crypto::nonce::Nonce;
// use simulator::one_real_node_cluster::*;
// use tezedge_state::proposer::TezedgeProposerConfig;
// use tezedge_state::{sample_tezedge_state, TezedgeConfig, TezedgeState};
// use tezos_messages::p2p::encoding::connection::ConnectionMessage;
// use tezos_messages::p2p::encoding::prelude::NetworkVersion;

// const PEER_COUNT: usize = 2;

// fn default_state(initial_time: Instant) -> TezedgeState {
//     sample_tezedge_state::build(
//         initial_time,
//         TezedgeConfig {
//             port: 100,
//             disable_mempool: true,
//             private_node: true,
//             disable_quotas: true,
//             disable_blacklist: false,
//             min_connected_peers: 1,
//             max_connected_peers: 2,
//             max_pending_peers: 2,
//             max_potential_peers: 10,
//             periodic_react_interval: Duration::from_millis(250),
//             reset_quotas_interval: Duration::from_secs(5),
//             peer_blacklist_duration: Duration::from_secs(30 * 60),
//             peer_timeout: Duration::from_secs(8),
//             pow_target: 0.0,
//         },
//     )
// }

// fn default_cluster() -> OneRealNodeCluster {
//     let initial_time = Instant::now();
//     OneRealNodeCluster::new(
//         initial_time,
//         TezedgeProposerConfig {
//             wait_for_events_timeout: Some(Duration::from_millis(250)),
//             events_limit: 1024,
//         },
//         default_state(initial_time),
//     )
// }

// #[derive(Debug, Clone)]
// enum MessageType {
//     Connection,
//     Metadata,
//     Ack,
// }

// #[derive(Debug, Clone)]
// enum NetworkEventType {
//     IncomingConnect,
//     // OutgoingConnect,

//     ReadyPartialRecv(MessageType),
//     ReadyFullRecv(MessageType),

//     ReadyPartialSend(MessageType),
//     ReadyFullSend(MessageType),

//     Disconnected,
// }

// impl NetworkEventType {
//     pub fn from_index(index: usize) -> Self {
//         match index {
//             0 => Self::IncomingConnect,
//             // 1 => Self::OutgoingConnect,

//             _ => unimplemented!(),
//         }
//     }
// }

// struct ScenarioGenerator {
//     peer_count: usize,
//     message_count: usize,
//     cluster: OneRealNodeCluster,
// }

// impl ScenarioGenerator {
//     fn new(peer_count: usize, message_count: usize) -> Self {
//         let mut cluster = default_cluster();
//         for _ in 0..peer_count {
//             cluster.init_new_fake_peer();
//         }

//         Self {
//             peer_count,
//             message_count,
//             cluster,
//         }
//     }

//     fn run_event_groups<const N: usize>(
//         &self,
//         initial_time: Instant,
//         event_groups: &[[Event<SimulatedNetworkEvent>; N]]
//     ) {
//         let num_of_disconnects = event_groups.iter()
//             .filter(|x| {
//                 if let Event::Network(event) = &x[0] {
//                     return matches!(event.event_type, NetworkEventType::Disconnected);
//                 }
//                 false
//             })
//             .count();
//         let config = TezedgeConfig {
//         };

//         let mut proposer = TezedgeProposer::new(
//             TezedgeProposerConfig {
//                 wait_for_events_timeout: Some(Duration::from_millis(250)),
//                 events_limit: 1024,
//             },
//             tezedge_state::sample_tezedge_state::build(initial_time, config.clone()),
//             // capacity is changed by events_limit.
//             SimulatedEvents::new(),
//             SimulatedManager::new(event_groups),
//         );

//         let mut i = 0;
//         while !proposer.manager.is_finished() {
//             proposer.make_progress();
//             let stats = proposer.state.stats();
//             assert!(stats.potential_peers_len <= config.max_potential_peers as usize);
//             assert!(stats.pending_peers_len <= config.max_pending_peers as usize);
//             assert!(stats.connected_peers_len <= config.max_connected_peers as usize);

//             assert!(proposer.manager.connected_peers.len() <= config.max_pending_peers as usize);
//             i += 1;
//         }
//         let stats = proposer.state.stats();

//         assert_eq!(stats.connected_peers_len, self.peer_count - num_of_disconnects);
//         assert_eq!(stats.blacklisted_peers_len, num_of_disconnects);
//         assert_eq!(stats.pending_peers_len, 0);
//     }

//     fn run_scenario(&self, scenario: &[Option<(usize, usize)>]) {
//         let scenario = scenario
//             .iter()
//             .filter_map(|x| x.clone())
//             .collect::<Vec<(usize, usize)>>();

//         'disconnect_idx_loop: for disconnect_idx in 1..=scenario.len() {
//             for disconnect_peer_i in 0..self.peer_count {
//                 let disconnect_peer_id = FakePeerId::new_unchecked(disconnect_peer_i);
//                 let mut cluster = self.cluster.clone();

//                 for i in 0..scenario.len() {
//                     if disconnect_idx == i {
//                         if !cluster.get_peer(disconnect_peer_id).is_connected() {
//                             continue 'disconnect_idx_loop;
//                         }
//                         cluster
//                             .advance_time_ms(100)
//                             .disconnect_peer(disconnect_peer_id);
//                     }
//                     if disconnect_idx >= i && disconnect_peer_i == scenario[i].0 {
//                         continue;
//                     }
//                     let peer_id = FakePeerId::new_unchecked(scenario[i].0);
//                     match scenario[i].1 {
//                         0 => {
//                             cluster
//                                 .connect_to_node(peer_id)
//                                 .unwrap()
//                                 .advance_time_ms(100);
//                         }
//                         1 => {
//                             let peer = cluster.get_peer(peer_id);
//                             let conn_msg = peer.default_conn_msg(Nonce::random(), NetworkVersion::new("TEZOS_MAINNET", , p2p_version))
//                             .send_conn_msg(ConnectionMessage::try_new(
//                                 12345, proof_of_work_stamp, message_nonce, version))
//                         }
//             1 => Self::ReadyPartialRecv(MessageType::Connection),
//             2 => Self::ReadyFullRecv(MessageType::Connection),

//             3 => Self::ReadyPartialSend(MessageType::Connection),
//             4 => Self::ReadyFullSend(MessageType::Connection),

//             5 => Self::ReadyPartialRecv(MessageType::Metadata),
//             6 => Self::ReadyFullRecv(MessageType::Metadata),

//             7 => Self::ReadyPartialSend(MessageType::Metadata),
//             8 => Self::ReadyFullSend(MessageType::Metadata),

//             9 => Self::ReadyPartialRecv(MessageType::Ack),
//             10 => Self::ReadyFullRecv(MessageType::Ack),

//             11 => Self::ReadyPartialSend(MessageType::Ack),
//             12 => Self::ReadyFullSend(MessageType::Ack),
//                     }
//                     cluster.get_peer(peer_id).is_connected()
//                 }
//             }
//         }

//         let mut event_groups = vec![[Event::Network(disconnect_event)]];

//         event_groups.extend(scenario.iter()
//             .enumerate()
//             .filter_map(|(index, val)| {
//                 val.map(|(peer_index, message_index)| {
//                     [Event::Network(SimulatedNetworkEvent {
//                         event_type: NetworkEventType::from_index(message_index),
//                         from: peer_index as u64,
//                         time: initial_time + Duration::from_millis(100 * (index + 1) as u64),
//                     })]
//                 })
//             }));

//         for index in 0..(event_groups.len() - 1) {
//             event_groups.swap(index, index + 1);
//             if let [Event::Network(prev_event)] = &event_groups[index] {
//                 let from = prev_event.from;
//                 let time = prev_event.time;

//                 if let [Event::Network(event)] = &mut event_groups[index + 1] {
//                     event.time = time + Duration::from_millis(50);
//                     event.from = from;
//                 } else {
//                     unreachable!()
//                 }

//                 self.run_event_groups(initial_time, &event_groups);
//             }
//         }

//         // remove disconnect event.
//         event_groups.pop();
//         self.run_event_groups(initial_time, &event_groups);
//     }

//     #[inline(always)]
//     fn min_index(&self, peer_index: usize, message_index: usize) -> usize {
//         peer_index + message_index
//     }

//     #[inline(always)]
//     fn max_index(&self, peer_index: usize, message_index: usize) -> usize {
//         (self.peer_count * self.message_count) - self.message_count + message_index + 1
//     }

//     fn run_scenarios(
//         &self,
//         scenario: &mut Vec<Option<(usize, usize)>>,
//         peer_index: usize,
//         message_index: usize,
//         start_i: usize,
//     ) {
//         let min_index = start_i.max(self.min_index(peer_index, message_index));

//         for i in min_index..self.max_index(peer_index, message_index) {
//             if scenario[i].is_none() {
//                 scenario[i] = Some((peer_index, message_index));
//                 if message_index == self.message_count - 1 {
//                     if peer_index == self.peer_count - 1 {
//                         self.run_scenario(scenario);
//                     }
//                     self.run_scenarios(scenario, peer_index + 1, 0, 0);
//                 } else {
//                     self.run_scenarios(scenario, peer_index, message_index + 1, i + 1);
//                 }
//                 scenario[i] = None;
//             }
//         }
//     }

//     pub fn run(&self) {
//         self.run_scenarios(
//             &mut vec![None; self.peer_count * self.message_count],
//             0,
//             0,
//             0,
//         );
//     }
// }

// #[test]
// fn simulate_disconnects() {
//     ScenarioGenerator::new(PEER_COUNT, 13).run();

//     for _ in 0..10000 {
//         let peer_id = cluster.init_new_fake_peer();
//         if let Ok(_) = cluster.connect_to_node(peer_id) {
//             cluster.add_writable_event(peer_id, None);
//         }
//         cluster
//             .advance_time_ms(5)
//             .make_progress()
//             .assert_state();
//     }

//     cluster.add_tick_for_current_time();

//     while !cluster.is_done() {
//         cluster.make_progress();
//         cluster.assert_state();
//     }
// }
