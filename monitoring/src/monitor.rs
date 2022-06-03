// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use slog::{debug, info, warn, Logger};
use tezedge_actor_system::{actor::*, system::Timer};

use crypto::hash::ChainId;
use networking::network_channel::{NetworkChannelMsg, NetworkChannelRef, PeerMessageReceived};
use shell::subscription::subscribe_to_network_events;
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::PersistentStorage;
use storage::{BlockStorage, BlockStorageReader, ChainMetaStorage, OperationsMetaStorage};

use crate::websocket::ws_json_rpc::JsonRpcResponse;
use crate::websocket::ws_messages::{WebsocketMessage, WebsocketMessageWrapper};
use crate::websocket::RpcClient;
use crate::{monitors::*, websocket::ws_messages::PeerConnectionStatus};
use tezos_messages::Head;

/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub enum BroadcastSignal {
    PublishPeerStatistics,
    PublishBlocksStatistics,
    PeerUpdate(PeerConnectionStatus),
    PublishAll((RpcClient, Option<json_rpc_types::Id>)),
}

#[derive(Clone, Debug)]
pub struct LogStats;

pub type MonitorRef = ActorRef<MonitorMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg, LogStats)]
pub struct Monitor {
    persistent_storage: PersistentStorage,
    main_chain_id: ChainId,
    network_channel: NetworkChannelRef,
    /// Monitors
    peer_monitors: HashMap<SocketAddr, PeerMonitor>,
    bootstrap_monitor: BootstrapMonitor,
    blocks_monitor: BlocksMonitor,
    block_application_monitor: ApplicationMonitor,
    chain_monitor: ChainMonitor,

    /// Count of received messages from the last log
    actor_received_messages_count: usize,
}

impl Monitor {
    fn name() -> &'static str {
        "monitor-manager"
    }

    pub fn actor(
        sys: &impl ActorRefFactory,
        event_channel: NetworkChannelRef,
        persistent_storage: PersistentStorage,
        main_chain_id: ChainId,
    ) -> Result<MonitorRef, CreateError> {
        sys.actor_of_props::<Monitor>(
            Self::name(),
            Props::new_args((event_channel, persistent_storage, main_chain_id)),
        )
    }

    fn process_peer_message(&mut self, msg: PeerMessageReceived, log: &Logger) {
        use std::mem::size_of_val;
        use tezos_messages::p2p::encoding::peer::PeerMessage;

        if let PeerMessage::CurrentBranch(msg) = msg.message.message() {
            if msg.current_branch().current_head().level() > 0 {
                self.bootstrap_monitor
                    .set_level(msg.current_branch().current_head().level() as usize);
            }
        }

        if let Some(monitor) = self.peer_monitors.get_mut(&msg.peer_address) {
            // TODO: TE-190 - reimplement correctly, now not all messages are counted in (Ack, Metadata, ConnectionMessage is not involved)
            let size = if let Some(size_hint) = msg.message.size_hint() {
                *size_hint
            } else {
                debug!(log, "size_hint not available for received peer message"; "peer" => msg.peer_address.to_string());
                size_of_val(&msg.message)
            };
            monitor.incoming_bytes(size);
        } else {
            debug!(log, "Missing monitor for peer"; "peer" => msg.peer_address.to_string());
        }
    }

    fn get_and_clear_actor_received_messages_count(&mut self) -> usize {
        std::mem::replace(&mut self.actor_received_messages_count, 0)
    }
}

impl ActorFactoryArgs<(NetworkChannelRef, PersistentStorage, ChainId)> for Monitor {
    fn create_args(
        (event_channel, persistent_storage, main_chain_id): (
            NetworkChannelRef,
            PersistentStorage,
            ChainId,
        ),
    ) -> Self {
        // default empty monitors
        let chain_monitor = ChainMonitor::new();
        let blocks_monitor = BlocksMonitor::new(4096, 0);
        let bootstrap_monitor = BootstrapMonitor::initialize(0, 0);

        Self {
            persistent_storage,
            main_chain_id,
            network_channel: event_channel,
            peer_monitors: HashMap::new(),
            bootstrap_monitor,
            blocks_monitor,
            block_application_monitor: ApplicationMonitor::new(),
            chain_monitor,
            actor_received_messages_count: 0,
        }
    }
}

impl Actor for Monitor {
    type Msg = MonitorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_network_events(&self.network_channel, ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into(),
        );

        // recalculate stats from storage, before start processing any new message
        let (chain_monitor, blocks_monitor, bootstrap_monitor) =
            initialize_monitors(&self.persistent_storage, &self.main_chain_id);
        self.chain_monitor = chain_monitor;
        self.blocks_monitor = blocks_monitor;
        self.bootstrap_monitor = bootstrap_monitor;
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        info!(ctx.system.log(), "Monitoring started");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.actor_received_messages_count += 1;
        self.receive(ctx, msg, sender);
    }
}

impl Receive<BroadcastSignal> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: BroadcastSignal, _sender: Sender) {
        if let BroadcastSignal::PublishAll((client, id)) = msg {
            let msg = WebsocketMessageWrapper::multiple(vec![
                WebsocketMessage::IncomingTransfer {
                    payload: self.bootstrap_monitor.snapshot(),
                },
                WebsocketMessage::BlockStatus {
                    payload: self.blocks_monitor.snapshot(),
                },
                WebsocketMessage::BlockApplicationStatus {
                    payload: self.block_application_monitor.snapshot(),
                },
                WebsocketMessage::ChainStatus {
                    payload: self.chain_monitor.snapshot(),
                },
                WebsocketMessage::PeersMetrics {
                    payload: self
                        .peer_monitors
                        .values_mut()
                        .map(|monitor| monitor.snapshot())
                        .collect(),
                },
            ]);

            if let Some(client) = client {
                let res = match serde_json::value::to_raw_value(&msg) {
                    Ok(serialized) => {
                        JsonRpcResponse::result(json_rpc_types::Version::V2, serialized, id)
                    }
                    Err(_) => {
                        let error = json_rpc_types::Error::from_code(
                            json_rpc_types::ErrorCode::InternalError,
                        );
                        JsonRpcResponse::error(json_rpc_types::Version::V2, error, id)
                    }
                };
                let _ = client.send(res);
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _: Sender) {
        match msg {
            NetworkChannelMsg::PeerBootstrapped(peer_id, _, _) => {
                let previous = self.peer_monitors.insert(
                    peer_id.address,
                    PeerMonitor::new(peer_id.address, peer_id.public_key_hash.clone()),
                );
                if let Some(_previous) = previous {
                    warn!(ctx.system.log(), "Duplicate monitor found for peer"; "key" => peer_id.address.to_string());
                } else {
                    ctx.myself.tell(
                        BroadcastSignal::PeerUpdate(PeerConnectionStatus::connected(
                            peer_id.address.to_string(),
                        )),
                        None,
                    );
                }
            }
            NetworkChannelMsg::PeerMessageReceived(msg) => {
                self.process_peer_message(msg, &ctx.system.log())
            }
            NetworkChannelMsg::PeerDisconnected(peer) => {
                if self.peer_monitors.remove(&peer).is_some() {
                    ctx.myself.tell(
                        BroadcastSignal::PeerUpdate(PeerConnectionStatus::disconnected(
                            peer.to_string(),
                        )),
                        None,
                    );
                }
            }

            NetworkChannelMsg::NewCurrentHead(notification) => {
                // update stats for block applications
                self.chain_monitor
                    .process_block_application(notification.block.header.level());

                self.blocks_monitor.block_was_applied_by_protocol();
                self.block_application_monitor.block_was_applied(Head::new(
                    notification.block.hash.clone(),
                    notification.block.header.level(),
                    notification.block.header.fitness().clone(),
                ));
            }
            NetworkChannelMsg::BlockReceived(msg) => {
                // Update current max block count
                self.bootstrap_monitor.set_level(msg.level as usize);

                // Start tracking it in the blocks monitor
                self.blocks_monitor.accept_block();
                self.bootstrap_monitor.increase_headers_count();

                // update stats for block header
                self.chain_monitor.process_block_header(msg.level);
            }
            NetworkChannelMsg::AllBlockOperationsReceived(msg) => {
                self.bootstrap_monitor.increase_block_count();
                self.blocks_monitor.block_finished_downloading_operations();

                // update stats for block operations
                self.chain_monitor.process_block_operations(msg.level);
            }
            _ => {}
        }
    }
}

fn initialize_monitors(
    persistent_storage: &PersistentStorage,
    main_chain_id: &ChainId,
) -> (ChainMonitor, BlocksMonitor, BootstrapMonitor) {
    let mut chain_monitor = ChainMonitor::new();

    let block_storage = BlockStorage::new(persistent_storage);
    let operations_meta_storage = OperationsMetaStorage::new(persistent_storage);

    let mut downloaded_headers = 0;
    let mut downloaded_blocks = 0;

    // populate the monitors with the data from storage
    if let Ok(iter) = block_storage.iterator() {
        for key in iter {
            if let Ok(Some(header_with_hash)) = block_storage.get(&key) {
                let block_level = header_with_hash.header.level();
                chain_monitor.process_block_header(block_level);
                downloaded_headers += 1;

                if let Ok(is_complete) = operations_meta_storage.is_complete(&header_with_hash.hash)
                {
                    if is_complete {
                        chain_monitor.process_block_operations(block_level);
                        downloaded_blocks += 1;
                    }
                }
            }
        }
    }

    let current_head_level = if let Ok(Some(head)) =
        ChainMetaStorage::new(persistent_storage).get_current_head(main_chain_id)
    {
        *head.level()
    } else {
        0
    };

    for level in 0..current_head_level {
        chain_monitor.process_block_application(level)
    }

    let bootstrap_monitor = BootstrapMonitor::initialize(downloaded_blocks, downloaded_headers);
    let block_monitor = BlocksMonitor::new(4096, downloaded_blocks);

    (chain_monitor, block_monitor, bootstrap_monitor)
}

impl Receive<LogStats> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _: LogStats, _: Sender) {
        info!(ctx.system.log(), "Monitoring info";
                   "actor_received_messages_count" => self.get_and_clear_actor_received_messages_count(),
                   "peers_count" => self.peer_monitors.len(),
        );
    }
}
