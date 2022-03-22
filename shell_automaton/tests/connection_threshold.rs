use std::{
    io::Write,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use shell_automaton::{
    config::default_test_config,
    peer::{
        connection::{
            closed::PeerConnectionClosedAction,
            incoming::{
                accept::{
                    PeerConnectionIncomingAcceptAction, PeerConnectionIncomingAcceptSuccessAction,
                    PeerConnectionIncomingRejectedAction, PeerConnectionIncomingRejectedReason,
                },
                PeerConnectionIncomingSuccessAction,
            },
            outgoing::{
                PeerConnectionOutgoingInitAction, PeerConnectionOutgoingPendingAction,
                PeerConnectionOutgoingRandomInitAction, PeerConnectionOutgoingSuccessAction,
            },
            PeerConnectionState,
        },
        disconnection::{PeerDisconnectAction, PeerDisconnectedAction},
        PeerToken,
    },
    peers::{add::PeersAddIncomingPeerAction, remove::PeersRemoveAction},
    reducer, ActionId, ActionWithMeta, Config, State,
};

macro_rules! peer_actions {
    ($count:expr, { $($rest:tt)* }) => {
        {
            let mut actions = Vec::new();
            peer_actions!(_impl actions, $count, { $($rest)* });
            actions
        }
    };

    (_impl $actions:expr, $count:expr, { $action:ident { address $(, $($fields:tt)*)? }, $($rest:tt)* } ) => {
        $actions.extend((0..$count as u16).map(|i| peer_actions!(address $action, i, $($($fields)*)?)));
        peer_actions!(_impl $actions, $count, { $($rest)* });
    };

    (_impl $actions:expr, $count:expr, { $action:ident { token_address $(, $($fields:tt)*)? }, $($rest:tt)* } ) => {
        $actions.extend((0..$count as u16).map(|i| peer_actions!(token_address $action, i, $($($fields)*)?)));
        peer_actions!(_impl $actions, $count, { $($rest)* });
    };

    (_impl $actions:expr, $count:expr, { $action:ident $({ $($fields:tt)* })?, $($rest:tt)* } ) => {
        $actions.push($action { $($($fields)*)? } .into());
        peer_actions!(_impl $actions, $count, { $($rest)* });
    };

    (_impl $actions:expr, $count:expr, {, }) => {};
    (_impl $actions:expr, $count:expr, { }) => {};

    (address $action:ident, $i:expr, $($fields:tt)*) => {
        $action { address: peer_actions!(address $i), $($fields)* }.into()
    };

    (token_address $action:ident, $i:expr, $($fields:tt)*) => {
        $action { token: peer_actions!(token $i), address: peer_actions!(address $i), $($fields)* }.into()
    };

    (address $i:expr) => {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), $i as u16)
    };

    (token $i:expr) => {
        PeerToken::new_unchecked($i as usize)
    };
}

fn prepare_actions(count: usize) -> Vec<ActionWithMeta> {
    let actions = peer_actions!(count, {
            PeersAddIncomingPeerAction { token_address },
            PeersRemoveAction { address },
            PeerConnectionIncomingAcceptAction,
            //PeerConnectionIncomingAcceptErrorAction { reason:  },
            PeerConnectionIncomingRejectedAction { token_address, reason: PeerConnectionIncomingRejectedReason::PeersConnectedMaxBoundReached },
            PeerConnectionIncomingAcceptSuccessAction { token_address },

            PeerConnectionIncomingSuccessAction { address },

            PeerConnectionOutgoingRandomInitAction,
            PeerConnectionOutgoingInitAction { address },
            PeerConnectionOutgoingPendingAction { token_address },
    //        PeerConnectionOutgoingErrorAction { token_address },
            PeerConnectionOutgoingSuccessAction { address },

            PeerConnectionClosedAction { address },

            PeerDisconnectAction { address },
            PeerDisconnectedAction { address },
        });
    actions
        .into_iter()
        .map(|action| ActionWithMeta {
            action,
            id: ActionId::new_unchecked(0),
            depth: 0,
        })
        .collect()
}

mod state_explorer;

#[derive(Debug, Clone, PartialEq, Eq, Hash, strum_macros::AsRefStr)]
enum PeerState {
    Potential,
    IncomingPending,
    IncomingSuccess,
    OutgoingIdle,
    OutgoingPending,
    OutgoingError,
    OutgoingSuccess,
    Connected,
    Disconnecting,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Peer {
    address: SocketAddr,
    state: PeerState,
}

impl std::fmt::Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.address, self.state.as_ref())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ConnectedPeers {
    peers: Vec<Peer>,
    max: usize,
}

impl From<State> for ConnectedPeers {
    fn from(state: State) -> Self {
        let mut peers: Vec<_> = state
            .peers.iter()
            .map(|(address, peer)| Peer {
                address: *address,
                state: match &peer.status {
                    shell_automaton::peer::PeerStatus::Potential => PeerState::Potential,
                    shell_automaton::peer::PeerStatus::Connecting(PeerConnectionState::Incoming(state)) => match state {
                        shell_automaton::peer::connection::incoming::PeerConnectionIncomingState::Pending { .. } => PeerState::IncomingPending,
                        shell_automaton::peer::connection::incoming::PeerConnectionIncomingState::Success { .. } => PeerState::IncomingSuccess,
                        shell_automaton::peer::connection::incoming::PeerConnectionIncomingState::Error { .. } => PeerState::Error,
                    },
                    shell_automaton::peer::PeerStatus::Connecting(PeerConnectionState::Outgoing(state)) => match state {
                        shell_automaton::peer::connection::outgoing::PeerConnectionOutgoingState::Idle { .. } => PeerState::OutgoingIdle,
                        shell_automaton::peer::connection::outgoing::PeerConnectionOutgoingState::Pending { .. } => PeerState::OutgoingPending,
                        shell_automaton::peer::connection::outgoing::PeerConnectionOutgoingState::Error { .. } => PeerState::OutgoingError,
                        shell_automaton::peer::connection::outgoing::PeerConnectionOutgoingState::Success { .. } => PeerState::OutgoingSuccess,
                    },
                    shell_automaton::peer::PeerStatus::Disconnecting(_) => PeerState::Disconnecting,
                    shell_automaton::peer::PeerStatus::Disconnected => PeerState::Disconnected,
                    _ => PeerState::Connected,
                },
            })
            .collect();
        peers.sort_by_key(|peer| peer.address);
        ConnectedPeers {
            peers,
            max: state.config.peers_connected_max,
        }
    }
}

impl state_explorer::GenState for ConnectedPeers {
    fn within_bounds(&self) -> bool {
        let num = self.peers.iter().fold(0, |num, peer| match peer.state {
            PeerState::IncomingSuccess | PeerState::OutgoingSuccess | PeerState::Connected => {
                num + 1
            }
            _ => num,
        });
        // panic if we have more connected peers that allowed
        assert!(num <= self.max);
        // limit the number of peers being processed
        num <= self.max * 2
    }
}

pub fn test_config(max_connections: usize) -> Config {
    Config {
        peers_connected_max: max_connections,
        ..default_test_config()
    }
}

/// This test can be executed as follows:
/// ```bash
/// PEERS_NUM=6 MAX_CONNECTIONS=4 cargo test --test connection_threshold -- --nocapture --ignored
/// ```
///
/// To generate states diagram in SVG, run the following (with greater numbers will take forever to render graph):
/// ```bash
/// $ GRAPH_FILE_NAME=graph.dot PEERS_NUM=3 MAX_CONNECTIONS=2 cargo test --test connection_threshold -- --nocapture --ignored
/// $ dot -Tsvg graph.dot -o graph.svg
/// ```
#[ignore = "Long running simulation for manual execution"]
#[test]
fn connection_threshold_simulation() {
    let max_peers: usize = std::env::var("PEERS_NUM")
        .map(|s| {
            s.parse::<usize>()
                .expect("cannot parse PEERS_NUM as integer")
        })
        .unwrap_or(8);
    let max_connections: usize = std::env::var("MAX_CONNECTIONS")
        .map(|s| {
            s.parse::<usize>()
                .expect("cannot parse MAX_CONNECTIONS as integer")
        })
        .unwrap_or(5);

    let actions = prepare_actions(max_peers);
    let explorer = state_explorer::StateExplorer::<_, _, ConnectedPeers>::new(
        State::new(test_config(max_connections)),
        actions,
        |s, a| {
            let mut s = s.clone();
            reducer(&mut s, a);
            Some(s)
        },
    );
    let graph = explorer.explore();
    if let Ok(name) = std::env::var("GRAPH_FILE_NAME") {
        dump_graph(&name, graph);
    }
}

fn dump_graph(name: &str, graph: state_explorer::Graph<ActionWithMeta, ConnectedPeers>) {
    std::fs::File::create(name)
        .and_then(|mut f| {
            writeln!(f, "digraph ConnectionThreshold {{")?;
            for (i, (state, transitions)) in graph.states_transitions.into_iter().enumerate() {
                writeln!(
                    f,
                    r#"State_{}[label="{}"]"#,
                    i,
                    state.peers.iter().fold(String::new(), |mut s, p| {
                        use std::fmt::Write;
                        write!(s, "{}\n", p).unwrap();
                        s
                    })
                )?;
                for (action, state) in transitions.transitions {
                    writeln!(
                        f,
                        r#"State_{} -> State_{}[label="{}"]"#,
                        i,
                        state,
                        graph.actions[action].action.kind()
                    )?;
                }
            }
            writeln!(f, "}}")?;
            Ok(())
        })
        .expect("cannot write graph");
}
