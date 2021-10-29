// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! PeerBranchBootstrapper is actor, which is responsible to download branches from peers
//! and schedule downloaded blocks for block application.
//! PeerBranchBootstrapper operates just for one chain_id.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use slog::{debug, info, warn, Logger};
use tezedge_actor_system::actors::*;

use crypto::hash::{BlockHash, ChainId};
use networking::PeerId;
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::chain_feeder::{ApplyBlockDone, ApplyBlockFailed};
use crate::chain_manager::ChainManagerRef;
use crate::state::bootstrap_state::{
    AddBranchState, BootstrapState, BootstrapStateConfiguration, InnerBlockState,
};
use crate::state::data_requester::DataRequesterRef;
use crate::state::peer_state::DataQueues;
use crate::state::synchronization_state::PeerBranchSynchronizationDone;
use crate::subscription::subscribe_to_actor_terminated;

/// After this interval, we will check peers, if no activity is done on any pipeline
/// So if peer does not change any branch bootstrap, we will disconnect it
const STALE_BOOTSTRAP_PEER_INTERVAL: Duration = Duration::from_secs(45);

/// Constatnt for rescheduling of processing bootstrap pipelines
const SCHEDULE_ONE_TIMER_DELAY: Duration = Duration::from_secs(15);

/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);

/// Message commands [`PeerBranchBootstrapper`] to disconnect peer if any of bootstrapping pipelines are stalled
#[derive(Clone, Debug)]
pub struct DisconnectStalledBootstraps;

#[derive(Clone, Debug)]
pub struct CleanPeerData(pub Arc<ActorUri>);

#[derive(Clone, Debug)]
pub struct LogStats;

#[derive(Clone, Debug)]
pub struct StartBranchBootstraping {
    peer_id: Arc<PeerId>,
    peer_queues: Arc<DataQueues>,
    chain_id: Arc<ChainId>,
    last_applied_block: BlockHash,
    missing_history: Vec<BlockHash>,
    to_level: Level,
}

impl StartBranchBootstraping {
    pub fn new(
        peer_id: Arc<PeerId>,
        peer_queues: Arc<DataQueues>,
        chain_id: Arc<ChainId>,
        last_applied_block: BlockHash,
        missing_history: Vec<BlockHash>,
        to_level: Level,
    ) -> Self {
        Self {
            peer_id,
            peer_queues,
            chain_id,
            last_applied_block,
            missing_history,
            to_level,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdateBranchBootstraping {
    peer_id: Arc<PeerId>,
    current_head: BlockHeaderWithHash,
}

impl UpdateBranchBootstraping {
    pub fn new(peer_id: Arc<PeerId>, current_head: BlockHeaderWithHash) -> Self {
        Self {
            peer_id,
            current_head,
        }
    }
}

/// This message should be trriggered, when all operations for the block are downloaded
#[derive(Clone, Debug)]
pub struct UpdateOperationsState {
    block_hash: BlockHash,
    peer_id: Arc<PeerId>,
}

impl UpdateOperationsState {
    pub fn new(block_hash: BlockHash, peer_id: Arc<PeerId>) -> Self {
        Self {
            block_hash,
            peer_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct UpdateBlockState {
    block_hash: BlockHash,
    new_state: InnerBlockState,
    peer_id: Arc<PeerId>,
}

impl UpdateBlockState {
    pub fn new(block_hash: BlockHash, new_state: InnerBlockState, peer_id: Arc<PeerId>) -> Self {
        Self {
            block_hash,
            new_state,
            peer_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PingBootstrapPipelinesProcessing {
    peer_id: Option<Arc<PeerId>>,
}

pub type PeerBranchBootstrapperRef = ActorRef<PeerBranchBootstrapperMsg>;

#[actor(
    StartBranchBootstraping,
    UpdateBranchBootstraping,
    PingBootstrapPipelinesProcessing,
    UpdateBlockState,
    UpdateOperationsState,
    ApplyBlockDone,
    ApplyBlockFailed,
    DisconnectStalledBootstraps,
    CleanPeerData,
    LogStats,
    SystemEvent
)]
pub struct PeerBranchBootstrapper {
    chain_id: Arc<ChainId>,
    bootstrap_state: BootstrapState,
    /// Count of received messages from the last log
    actor_received_messages_count: usize,

    /// parent reference to ChainManager
    chain_manager: Arc<ChainManagerRef>,

    /// We dont want to stuck pipelines, so we schedule ping for all pipelines to continue
    /// If we scheduled one ping, we dont need to schedule another until the first one is resolved.
    /// This is kind of optimization to prevenet overloading processing of all pipelines, when it is not necessery.
    /// And another things is that, we can randomly schedule block/operation download request,
    /// for example when we find 10 peers with the same request, we schedule just random number,
    /// and with this ping check other can continue, even if they did not receive data.
    is_already_scheduled_ping_for_process_all_bootstrap_pipelines: bool,
}

impl PeerBranchBootstrapper {
    /// Create new actor instance.
    pub fn actor(
        sys: &ActorSystem,
        chain_id: Arc<ChainId>,
        requester: DataRequesterRef,
        chain_manager: ChainManagerRef,
        cfg: BootstrapStateConfiguration,
    ) -> Result<PeerBranchBootstrapperRef, CreateError> {
        sys.actor_of_props::<PeerBranchBootstrapper>(
            &format!("peer-branch-bootstrapper-{}", &chain_id.to_base58_check()),
            Props::new_args((chain_id, requester, chain_manager, cfg)),
        )
    }

    fn schedule_process_all_bootstrap_pipelines(
        &mut self,
        ctx: &Context<PeerBranchBootstrapperMsg>,
    ) {
        // if not scheduled, schedule one
        if !self.is_already_scheduled_ping_for_process_all_bootstrap_pipelines {
            self.is_already_scheduled_ping_for_process_all_bootstrap_pipelines = true;
            ctx.schedule_once(
                SCHEDULE_ONE_TIMER_DELAY,
                ctx.myself(),
                None,
                PingBootstrapPipelinesProcessing {
                    // here we dont want to select peer
                    peer_id: None,
                },
            );
        }
    }

    fn schedule_process_bootstrap_pipeline_for_peer(
        &mut self,
        ctx: &Context<PeerBranchBootstrapperMsg>,
        peer_id: Arc<PeerId>,
    ) {
        // if not scheduled for peer, schedule one
        if let Some(peer_state) = self.bootstrap_state.peers.get_mut(peer_id.peer_ref.uri()) {
            if !peer_state.is_already_scheduled_ping_for_process_all_bootstrap_pipelines {
                peer_state.is_already_scheduled_ping_for_process_all_bootstrap_pipelines = true;
                // schedule with delay
                let mut rng = rand::thread_rng();
                let delay_in_secs = rng.gen_range(1, 15);
                ctx.schedule_once(
                    Duration::from_secs(delay_in_secs),
                    ctx.myself(),
                    None,
                    PingBootstrapPipelinesProcessing {
                        // here we want to select peer
                        peer_id: Some(peer_id),
                    },
                );
            }
        }
    }

    fn get_and_clear_actor_received_messages_count(&mut self) -> usize {
        std::mem::replace(&mut self.actor_received_messages_count, 0)
    }

    fn clean_peer_data(&mut self, actor: &ActorUri) {
        self.bootstrap_state.clean_peer_data(actor);
    }

    fn process_bootstrap_pipelines(
        &mut self,
        filter_peer: Arc<PeerId>,
        ctx: &Context<PeerBranchBootstrapperMsg>,
        log: &Logger,
    ) {
        let PeerBranchBootstrapper {
            bootstrap_state,
            chain_id,
            chain_manager,
            ..
        } = self;

        // schedule missing blocks for download
        bootstrap_state.schedule_blocks_to_download(&filter_peer, log);

        // schedule missing operations for download
        bootstrap_state.schedule_operations_to_download(&filter_peer, log);

        // schedule downloaded blocks as batch for block application
        bootstrap_state.schedule_blocks_for_apply(&filter_peer, log);

        // try to fire latest batch for block application (if possible)
        bootstrap_state.try_call_apply_block_batch(chain_id, chain_manager, &ctx.myself);

        // check, if we completed any pipeline
        bootstrap_state.check_bootstrapped_branches(&Some(filter_peer), log);
    }

    fn log_state_and_stats(&mut self, title: &str, log: &Logger) {
        let processing_blocks_scheduled_for_apply = self.bootstrap_state.blocks_scheduled_count();
        let (
            processing_peer_branches,
            (
                processing_block_intervals,
                processing_block_intervals_open,
                processing_block_intervals_scheduled_for_apply,
            ),
        ) = self.bootstrap_state.block_intervals_stats();

        // count queue batches
        let (
            apply_block_batch_count,
            apply_block_batch_blocks_count,
            apply_block_batch_in_progress,
        ) = self.bootstrap_state.apply_block_batch_queue_stats();

        info!(log, "{}", title;
                   "actor_received_messages_count" => self.get_and_clear_actor_received_messages_count(),
                   "peers_count" => self.bootstrap_state.peers_count(),
                   "peers_branches" => processing_peer_branches,
                   "peers_branches_level" => {
                        itertools::join(
                            &self.bootstrap_state
                                .peers_branches_level()
                                .iter()
                                .map(|(to_level, block)| format!("{} ({})", to_level, block.to_base58_check()))
                                .collect::<HashSet<_>>(),
                            ", ",
                        )
                   },
                   "block_intervals" => processing_block_intervals,
                   "block_intervals_open" => processing_block_intervals_open,
                   "block_intervals_scheduled_for_apply" => processing_block_intervals_scheduled_for_apply,
                   "block_intervals_next_lowest_missing_blocks" => {
                        self.bootstrap_state
                            .next_lowest_missing_blocks()
                            .iter()
                            .map(|(interval_idx, block)| format!("{} (i:{})", block.to_base58_check(), interval_idx))
                            .collect::<Vec<_>>().join(", ")
                   },
                   "blocks_scheduled_for_apply" => processing_blocks_scheduled_for_apply,
                   "apply_block_batch_count" => apply_block_batch_count,
                   "apply_block_batch_blocks_count" => apply_block_batch_blocks_count,
                   "apply_block_batch_in_progress" => apply_block_batch_in_progress,
        );
    }
}

impl
    ActorFactoryArgs<(
        Arc<ChainId>,
        DataRequesterRef,
        ChainManagerRef,
        BootstrapStateConfiguration,
    )> for PeerBranchBootstrapper
{
    fn create_args(
        (chain_id, requester, chain_manager, cfg): (
            Arc<ChainId>,
            DataRequesterRef,
            ChainManagerRef,
            BootstrapStateConfiguration,
        ),
    ) -> Self {
        let chain_manager = Arc::new(chain_manager);
        let peer_branch_synchronization_done_callback = {
            let chain_manager = chain_manager.clone();
            Box::new(move |msg: PeerBranchSynchronizationDone| {
                chain_manager.tell(msg, None);
            })
        };

        PeerBranchBootstrapper {
            chain_id,
            bootstrap_state: BootstrapState::new(
                requester,
                peer_branch_synchronization_done_callback,
                cfg,
            ),
            actor_received_messages_count: 0,
            is_already_scheduled_ping_for_process_all_bootstrap_pipelines: false,
            chain_manager,
        }
    }
}

impl Actor for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            STALE_BOOTSTRAP_PEER_INTERVAL,
            STALE_BOOTSTRAP_PEER_INTERVAL,
            ctx.myself(),
            None,
            DisconnectStalledBootstraps.into(),
        );

        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into(),
        );
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        info!(ctx.system.log(), "Peer branch bootstrapped started";
                                "chain_id" => self.chain_id.to_base58_check());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        let timer = Instant::now();
        self.actor_received_messages_count += 1;
        let msg_type = match msg {
            PeerBranchBootstrapperMsg::StartBranchBootstraping(_) => "StartBranchBootstraping",
            PeerBranchBootstrapperMsg::UpdateBranchBootstraping(_) => "UpdateBranchBootstraping",
            PeerBranchBootstrapperMsg::PingBootstrapPipelinesProcessing(_) => {
                "PingBootstrapPipelinesProcessing"
            }
            PeerBranchBootstrapperMsg::UpdateBlockState(_) => "UpdateBlockState",
            PeerBranchBootstrapperMsg::UpdateOperationsState(_) => "UpdateOperationsState",
            PeerBranchBootstrapperMsg::ApplyBlockDone(_) => "ApplyBlockDone",
            PeerBranchBootstrapperMsg::ApplyBlockFailed(_) => "ApplyBlockFailed",
            PeerBranchBootstrapperMsg::DisconnectStalledBootstraps(_) => {
                "DisconnectStalledBootstraps"
            }
            PeerBranchBootstrapperMsg::CleanPeerData(_) => "CleanPeerData",
            PeerBranchBootstrapperMsg::LogStats(_) => "LogStats",
            PeerBranchBootstrapperMsg::SystemEvent(_) => "SystemEvent",
        };

        self.receive(ctx, msg, sender);

        if timer.elapsed() > Duration::from_secs(1) {
            warn!(
                ctx.system.log(),
                "Peer branch bootstrapper too slow action processing: {} - {:?}",
                msg_type,
                timer.elapsed()
            );
        }
    }

    fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
        if let SystemMsg::Event(evt) = msg {
            self.actor_received_messages_count += 1;
            self.receive(ctx, evt, sender);
        }
    }
}

impl Receive<SystemEvent> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: SystemEvent, _: Option<BasicActorRef>) {
        if let SystemEvent::ActorTerminated(evt) = msg {
            self.clean_peer_data(evt.actor.uri());
        }
    }
}

impl Receive<StartBranchBootstraping> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: StartBranchBootstraping,
        _: Option<BasicActorRef>,
    ) {
        let log = ctx.system.log();
        debug!(log, "Start branch bootstrapping process";
            "last_applied_block" => msg.last_applied_block.to_base58_check(),
            "missing_history" => msg.missing_history
                .iter()
                .map(|b| b.to_base58_check())
                .collect::<Vec<String>>()
                .join(", "),
            "to_level" => &msg.to_level,
            "peer_id" => msg.peer_id.peer_id_marker.clone(), "peer_ip" => msg.peer_id.peer_address.to_string(), "peer" => msg.peer_id.peer_ref.name(), "peer_uri" => msg.peer_id.peer_ref.uri().to_string(),
        );

        // bootstrapper supports just one chain, if this will be issue, we need to create a new bootstrapper per chain_id
        if !self.chain_id.eq(&msg.chain_id) {
            warn!(log, "Branch is rejected, because of different chain_id";
                "peer_branch_bootstrapper_chain_id" => self.chain_id.to_base58_check(),
                "requested_branch_chain_id" => msg.chain_id.to_base58_check(),
                "last_applied_block" => msg.last_applied_block.to_base58_check(),
                "peer_id" => msg.peer_id.peer_id_marker.clone(), "peer_ip" => msg.peer_id.peer_address.to_string(), "peer" => msg.peer_id.peer_ref.name(), "peer_uri" => msg.peer_id.peer_ref.uri().to_string(),
            );
            return;
        }

        // add new branch (if possible)
        let timer = Instant::now();
        let result = self.bootstrap_state.add_new_branch(
            msg.peer_id.clone(),
            msg.peer_queues,
            msg.last_applied_block,
            msg.missing_history,
            msg.to_level,
            &log,
        );

        // process
        self.process_bootstrap_pipelines(msg.peer_id.clone(), ctx, &log);

        if let AddBranchState::Added(was_merged) = result {
            debug!(log, "Branch bootstrapping process started";
                       "started_in" => format!("{:?}", timer.elapsed()),
                       "to_level" => msg.to_level,
                       "new_branch" => if was_merged { "merged" } else { "created" },
                       "peer_id" => msg.peer_id.peer_id_marker.clone(), "peer_ip" => msg.peer_id.peer_address.to_string(), "peer" => msg.peer_id.peer_ref.name(), "peer_uri" => msg.peer_id.peer_ref.uri().to_string(),
            );
        }
    }
}

impl Receive<UpdateBranchBootstraping> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: UpdateBranchBootstraping,
        _: Option<BasicActorRef>,
    ) {
        // try to merge to existing branch (if possible)
        let was_updated = self
            .bootstrap_state
            .try_update_branch(msg.peer_id.clone(), &msg.current_head);

        if was_updated {
            let log = ctx.system.log();
            info!(log, "Branch bootstrapping process was updated with new block";
                       "new_block" => msg.current_head.hash.to_base58_check(),
                       "new_to_level" => msg.current_head.header.level(),
                       "peer_id" => msg.peer_id.peer_id_marker.clone(), "peer_ip" => msg.peer_id.peer_address.to_string(), "peer" => msg.peer_id.peer_ref.name(), "peer_uri" => msg.peer_id.peer_ref.uri().to_string(),
            );
            // process
            self.process_bootstrap_pipelines(msg.peer_id, ctx, &log);
        }
    }
}

impl Receive<PingBootstrapPipelinesProcessing> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        ping: PingBootstrapPipelinesProcessing,
        _: Option<BasicActorRef>,
    ) {
        if let Some(peer) = ping.peer_id {
            if let Some(peer_state) = self.bootstrap_state.peers.get_mut(peer.peer_ref.uri()) {
                peer_state.is_already_scheduled_ping_for_process_all_bootstrap_pipelines = false;
                // if we have ping for peer, we trigger processing
                self.process_bootstrap_pipelines(peer, ctx, &ctx.system.log());
            }
        } else {
            self.is_already_scheduled_ping_for_process_all_bootstrap_pipelines = false;
            // get all peers and schedule pings for them
            // we cannot trigger here process_bootstrap_pipelines for all peers,
            // because it will take so long time, so we split processing in time to different event
            let mut peers = self
                .bootstrap_state
                .peers
                .values()
                .map(|peer_state| peer_state.peer_id.clone())
                .collect::<Vec<_>>();
            peers.drain(..).for_each(|peer| {
                self.schedule_process_bootstrap_pipeline_for_peer(ctx, peer);
            });
        }
    }
}

impl Receive<CleanPeerData> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: CleanPeerData, _: Option<BasicActorRef>) {
        self.clean_peer_data(&msg.0);
    }
}

impl Receive<LogStats> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _: LogStats, _: Sender) {
        self.log_state_and_stats(
            "Peer branch bootstrapper processing info",
            &ctx.system.log(),
        );
    }
}

impl Receive<UpdateBlockState> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: UpdateBlockState,
        _: Option<BasicActorRef>,
    ) {
        let log = ctx.system.log();

        // process message
        let UpdateBlockState {
            block_hash,
            new_state,
            peer_id,
        } = msg;

        // process message
        self.bootstrap_state
            .block_downloaded(block_hash, new_state, &log);

        // process bootstrap
        self.process_bootstrap_pipelines(peer_id, ctx, &log);
    }
}

impl Receive<UpdateOperationsState> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: UpdateOperationsState,
        _: Option<BasicActorRef>,
    ) {
        // process message
        self.bootstrap_state
            .block_operations_downloaded(&msg.block_hash);

        // process
        self.process_bootstrap_pipelines(msg.peer_id, ctx, &ctx.system.log());
    }
}

impl Receive<ApplyBlockDone> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ApplyBlockDone, _: Option<BasicActorRef>) {
        let PeerBranchBootstrapper {
            bootstrap_state,
            chain_id,
            chain_manager,
            ..
        } = self;

        // process message
        let ApplyBlockDone {
            last_applied,
            mut permit,
        } = msg;
        bootstrap_state.block_applied(&last_applied, &ctx.system.log());

        // allow next call
        if let Some(permit) = permit.take() {
            drop(permit);
        }

        // try to fire latest batch for block application (if possible)
        bootstrap_state.try_call_apply_block_batch(chain_id, chain_manager, &ctx.myself);

        // schedule ping for other pipelines
        self.schedule_process_all_bootstrap_pipelines(ctx);
    }
}

impl Receive<ApplyBlockFailed> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: ApplyBlockFailed,
        _: Option<BasicActorRef>,
    ) {
        let log = ctx.system.log();
        self.log_state_and_stats(
            &format!(
                "Peer branch bootstrapper (apply block {} failed) - state before",
                msg.failed_block.to_base58_check()
            ),
            &log,
        );

        // process message
        let ApplyBlockFailed {
            failed_block,
            mut permit,
        } = msg;

        self.bootstrap_state.block_apply_failed(
            &failed_block,
            self.chain_id.as_ref().clone(),
            &log,
            |peer| {
                ctx.system.stop(peer.peer_ref.clone());
            },
        );

        // allow next call
        if let Some(permit) = permit.take() {
            drop(permit);
        }

        self.log_state_and_stats(
            &format!(
                "Peer branch bootstrapper (apply block {} failed) - state after",
                failed_block.to_base58_check()
            ),
            &log,
        );

        // schedule ping for other pipelines
        self.schedule_process_all_bootstrap_pipelines(ctx);
    }
}

impl Receive<DisconnectStalledBootstraps> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        _: DisconnectStalledBootstraps,
        _: Option<BasicActorRef>,
    ) {
        let log = ctx.system.log();

        let PeerBranchBootstrapper {
            bootstrap_state, ..
        } = self;

        bootstrap_state.check_bootstrapped_branches(&None, &log);
        bootstrap_state.check_stalled_peers(&log, |peer| {
            ctx.system.stop(peer.peer_ref.clone());
        });

        self.schedule_process_all_bootstrap_pipelines(ctx);
    }
}
