// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use riker::actors::*;
use slog::{debug, warn, Logger};

use crypto::hash::{BlockHash, ChainId};
use networking::PeerId;
use storage::{BlockMetaStorage, BlockMetaStorageReader, OperationsMetaStorage};
use tezos_messages::p2p::encoding::block_header::Level;

use crate::shell_channel::{ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::state::bootstrap_state::{BootstrapState, InnerBlockState};
use crate::state::data_requester::DataRequesterRef;
use crate::state::peer_state::DataQueues;
use crate::state::synchronization_state::PeerBranchSynchronizationDone;
use crate::state::StateError;
use crate::subscription::subscribe_to_actor_terminated;

/// After this timeout peer will be disconnected if no activity is done on any pipeline
/// So if peer does not change any branch bootstrap, we will disconnect it
const STALE_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(90);

/// If we have empty bootstrap pipelines for along time, we disconnect peer, means, peer is not provoding us a new current heads/branches
const MISSING_NEW_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs(60 * 2);

/// Constatnt for rescheduling of processing bootstrap pipelines
const NO_DATA_SCHEDULED_NEXT_SCHEDULE_ONE_TIMER_DELAY: Duration = Duration::from_secs(2);
const DATA_SCHEDULED_NEXT_SCHEDULE_ONE_TIMER_DELAY: Duration = Duration::from_millis(5);

/// Message commands [`PeerBranchBootstrapper`] to disconnect peer if any of bootstraping pipelines are stalled
#[derive(Clone, Debug)]
pub struct DisconnectStalledBootstraps {
    timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct StartBranchBootstraping {
    chain_id: Arc<ChainId>,
    last_applied_block: Arc<BlockHash>,
    missing_history: Vec<Arc<BlockHash>>,
    to_level: Arc<Level>,
}

impl StartBranchBootstraping {
    pub fn new(
        chain_id: Arc<ChainId>,
        last_applied_block: Arc<BlockHash>,
        missing_history: Vec<Arc<BlockHash>>,
        to_level: Arc<Level>,
    ) -> Self {
        Self {
            chain_id,
            last_applied_block,
            missing_history,
            to_level,
        }
    }
}

/// This message should be trriggered, when all operations for the block are downloaded
#[derive(Clone, Debug)]
pub struct UpdateOperationsState {
    block_hash: Arc<BlockHash>,
}

impl UpdateOperationsState {
    pub fn new(block_hash: Arc<BlockHash>) -> Self {
        Self { block_hash }
    }
}

#[derive(Clone, Debug)]
pub struct UpdateBlockState {
    block_hash: Arc<BlockHash>,
    predecessor_block_hash: Arc<BlockHash>,
    new_state: Arc<InnerBlockState>,
}

impl UpdateBlockState {
    pub fn new(
        block_hash: Arc<BlockHash>,
        predecessor_block_hash: Arc<BlockHash>,
        new_state: InnerBlockState,
    ) -> Self {
        Self {
            block_hash,
            predecessor_block_hash,
            new_state: Arc::new(new_state),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PingBootstrapPipelinesProcessing;

#[derive(Clone, Debug)]
pub struct BlockAlreadyApplied {
    pub block_hash: Arc<BlockHash>,
}

#[actor(
    StartBranchBootstraping,
    UpdateBlockState,
    UpdateOperationsState,
    DisconnectStalledBootstraps,
    BlockAlreadyApplied,
    PingBootstrapPipelinesProcessing,
    SystemEvent
)]
pub struct PeerBranchBootstrapper {
    peer: Arc<PeerId>,
    peer_queues: Arc<DataQueues>,
    queued_block_headers_for_apply: HashMap<Arc<BlockHash>, Instant>,

    shell_channel: ShellChannelRef,
    block_meta_storage: BlockMetaStorage,
    operations_meta_storage: OperationsMetaStorage,

    requester: DataRequesterRef,

    bootstrap_state: Vec<BootstrapState>,
    empty_bootstrap_state: Option<Instant>,

    max_bootstrap_interval_look_ahead_count: i8,
    max_bootstrap_branches_per_peer: usize,

    /// Indicates that we already scheduled process_bootstrap_pipelines, means we have already unprocessed message in mailbox
    /// So, we dont need to add it twice - this is kind if optimization, not to exhause mailbox with the same messages
    process_bootstrap_pipelines_triggered: AtomicBool,
}

pub type PeerBranchBootstrapperRef = ActorRef<PeerBranchBootstrapperMsg>;

impl PeerBranchBootstrapper {
    /// Create new actor instance.
    pub fn actor(
        sys: &ActorSystem,
        peer: Arc<PeerId>,
        peer_queues: Arc<DataQueues>,
        requester: DataRequesterRef,
        shell_channel: ShellChannelRef,
        block_meta_storage: BlockMetaStorage,
        operations_meta_storage: OperationsMetaStorage,
        max_bootstrap_interval_look_ahead_count: i8,
        max_bootstrap_branches_per_peer: usize,
    ) -> Result<PeerBranchBootstrapperRef, CreateError> {
        sys.actor_of_props::<PeerBranchBootstrapper>(
            &format!("{}-branch-bootstrap", &peer.peer_ref.name()),
            Props::new_args((
                peer,
                peer_queues,
                requester,
                shell_channel,
                block_meta_storage,
                operations_meta_storage,
                max_bootstrap_interval_look_ahead_count,
                max_bootstrap_branches_per_peer,
            )),
        )
    }

    /// We can receive parallely diferent event, and after each event we dont need to schedule this one, becuase it is unnecesseray:
    ///
    /// e1 (schedule pbpb1), e2 (schedule pbpb2), e3 (schedule pbpb3), pbpb1, pbpb2, pbpb3, e4 (schedule pbpb4), pbpb4
    ///
    /// we want it like this:
    ///
    /// e1 (schedule pbpb1), e2 (schedule pbpb2 - not needed), e3 (schedule pbpb3 - not needed), pbpb1, e4 (schedule pbpb4), pbpb4
    ///
    fn schedule_process_bootstrap_pipelines(
        &mut self,
        ctx: &Context<PeerBranchBootstrapperMsg>,
        schedule_at_delay: Duration,
    ) {
        // if not already scheduled, than schedule
        if !self
            .process_bootstrap_pipelines_triggered
            .load(Ordering::Acquire)
        {
            self.process_bootstrap_pipelines_triggered
                .store(true, Ordering::Release);

            ctx.schedule_once(
                schedule_at_delay,
                ctx.myself(),
                None,
                PingBootstrapPipelinesProcessing,
            );
        }
    }

    fn process_bootstrap_pipelines(
        &mut self,
        ctx: &Context<PeerBranchBootstrapperMsg>,
        log: &Logger,
    ) {
        // at first check apply blocks, because it is faster check,
        // and also it could remove lots of block, if find any already applied, so every next iterations will be shorter for process_data_download
        self.process_block_apply(ctx.myself(), log);

        // now we check and schedule missing data for download
        let was_data_download_scheduled = self.process_data_download(log);

        // lets schedule next run/processing
        self.schedule_process_bootstrap_pipelines(
            ctx,
            if was_data_download_scheduled {
                // if we have something more to download, then we check a next run
                DATA_SCHEDULED_NEXT_SCHEDULE_ONE_TIMER_DELAY
            } else {
                // we can relax more, if no data were scheduled
                NO_DATA_SCHEDULED_NEXT_SCHEDULE_ONE_TIMER_DELAY
            },
        );

        // check, if we completed any pipeline
        self.handle_resolved_bootstraps();
    }

    fn process_data_download(&mut self, log: &Logger) -> bool {
        let PeerBranchBootstrapper {
            peer,
            peer_queues,
            bootstrap_state,
            requester,
            block_meta_storage,
            operations_meta_storage,
            max_bootstrap_interval_look_ahead_count,
            ..
        } = self;

        let mut was_scheduled = false;
        bootstrap_state.iter_mut().for_each(|bootstrap| {
            // schedule next block downloading
            was_scheduled |= schedule_block_downloading(
                peer,
                peer_queues,
                bootstrap,
                requester,
                block_meta_storage,
                operations_meta_storage,
                *max_bootstrap_interval_look_ahead_count,
                log,
            );

            // schedule next block downloading
            was_scheduled |= schedule_operations_downloading(
                peer,
                peer_queues,
                bootstrap,
                requester,
                operations_meta_storage,
                *max_bootstrap_interval_look_ahead_count,
                log,
            );
        });
        was_scheduled
    }

    fn process_block_apply(&mut self, myself: PeerBranchBootstrapperRef, log: &Logger) {
        let PeerBranchBootstrapper {
            peer,
            bootstrap_state,
            block_meta_storage,
            queued_block_headers_for_apply,
            requester,
            ..
        } = self;

        bootstrap_state.iter_mut().for_each(|bootstrap| {
            // schedule next block for apply
            schedule_block_applying(
                peer,
                bootstrap,
                queued_block_headers_for_apply,
                block_meta_storage,
                requester,
                myself.clone(),
                log,
            );
        });
    }

    fn handle_resolved_bootstraps(&mut self) {
        let PeerBranchBootstrapper {
            bootstrap_state,
            peer,
            shell_channel,
            ..
        } = self;

        bootstrap_state.retain(|b| {
            if b.is_done() {
                shell_channel.tell(
                    Publish {
                        msg: ShellChannelMsg::PeerBranchSynchronizationDone(
                            PeerBranchSynchronizationDone::new(peer.clone(), b.to_level().clone()),
                        ),
                        topic: ShellChannelTopic::ShellCommands.into(),
                    },
                    None,
                );

                false
            } else {
                true
            }
        });

        if self.bootstrap_state.is_empty() && self.empty_bootstrap_state.is_none() {
            self.empty_bootstrap_state = Some(Instant::now());
        }
    }
}

impl
    ActorFactoryArgs<(
        Arc<PeerId>,
        Arc<DataQueues>,
        DataRequesterRef,
        ShellChannelRef,
        BlockMetaStorage,
        OperationsMetaStorage,
        i8,
        usize,
    )> for PeerBranchBootstrapper
{
    fn create_args(
        (
            peer,
            peer_queues,
            requester,
            shell_channel,
            block_meta_storage,
            operations_meta_storage,
            max_bootstrap_interval_look_ahead_count,
            max_bootstrap_branches_per_peer,
        ): (
            Arc<PeerId>,
            Arc<DataQueues>,
            DataRequesterRef,
            ShellChannelRef,
            BlockMetaStorage,
            OperationsMetaStorage,
            i8,
            usize,
        ),
    ) -> Self {
        PeerBranchBootstrapper {
            peer,
            peer_queues,
            requester,
            queued_block_headers_for_apply: Default::default(),
            bootstrap_state: Default::default(),
            shell_channel,
            block_meta_storage,
            operations_meta_storage,
            empty_bootstrap_state: None,
            process_bootstrap_pipelines_triggered: AtomicBool::new(false),
            max_bootstrap_interval_look_ahead_count,
            max_bootstrap_branches_per_peer,
        }
    }
}

impl Actor for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            STALE_BOOTSTRAP_TIMEOUT,
            STALE_BOOTSTRAP_TIMEOUT,
            ctx.myself(),
            None,
            DisconnectStalledBootstraps {
                timeout: STALE_BOOTSTRAP_TIMEOUT,
            }
            .into(),
        );
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }

    fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender);
        }
    }
}

impl Receive<SystemEvent> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemEvent,
        _sender: Option<BasicActorRef>,
    ) {
        if let SystemEvent::ActorTerminated(evt) = msg {
            if self.peer.peer_ref.uri().eq(evt.actor.uri()) {
                warn!(ctx.system.log(), "Stopping peer's branch bootstrapper, because peer is terminated";
                                        "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string());
                ctx.stop(ctx.myself());
            }
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
        debug!(ctx.system.log(), "Start bootstrapping process";
            "last_applied_block" => msg.last_applied_block.to_base58_check(),
            "missing_history" => msg.missing_history
                .iter()
                .map(|b| b.to_base58_check())
                .collect::<Vec<String>>()
                .join(", "),
            "to_level" => &msg.to_level,
            "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string(),
        );

        // check closed pipelines
        self.handle_resolved_bootstraps();

        if self.bootstrap_state.len() >= self.max_bootstrap_branches_per_peer {
            debug!(ctx.system.log(), "Peer has started already maximum ({}) pipeline, so we dont start new one", self.max_bootstrap_branches_per_peer;
                                    "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string());
            return;
        }

        // add new bootstrap
        self.bootstrap_state.push(BootstrapState::new(
            msg.chain_id,
            msg.last_applied_block,
            msg.missing_history,
            msg.to_level,
        ));
        self.empty_bootstrap_state = None;

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log())
    }
}

impl Receive<PingBootstrapPipelinesProcessing> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        _: PingBootstrapPipelinesProcessing,
        _: Option<BasicActorRef>,
    ) {
        // clean for next run
        self.process_bootstrap_pipelines_triggered
            .store(false, Ordering::Release);

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log());
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
        // process message
        self.bootstrap_state.iter_mut().for_each(|bootstrap| {
            bootstrap.block_downloaded(
                &msg.block_hash,
                &msg.new_state,
                msg.predecessor_block_hash.clone(),
            )
        });

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log())
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
        // check pipelines
        self.bootstrap_state.iter_mut().for_each(|bootstrap| {
            bootstrap.block_operations_downloaded(&msg.block_hash);
        });

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log())
    }
}

impl Receive<BlockAlreadyApplied> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: BlockAlreadyApplied,
        _: Option<BasicActorRef>,
    ) {
        // process message
        self.bootstrap_state.iter_mut().for_each(|bootstrap| {
            bootstrap.block_applied(&msg.block_hash);
        });

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log())
    }
}

impl Receive<DisconnectStalledBootstraps> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: DisconnectStalledBootstraps,
        _: Option<BasicActorRef>,
    ) {
        self.handle_resolved_bootstraps();

        // check for any stalled bootstrap
        let mut is_stalled = self
            .bootstrap_state
            .iter()
            .any(|bootstrap| bootstrap.is_stalled(msg.timeout));

        if let Some(empty_bootstrap_state) = self.empty_bootstrap_state {
            if empty_bootstrap_state.elapsed() > MISSING_NEW_BOOTSTRAP_TIMEOUT {
                warn!(ctx.system.log(), "Peer did not sent new curent_head/current_branch for a long time";
                    "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string(),
                );
                is_stalled = true;
            }
        }

        // if stalled, just disconnect peer
        if is_stalled {
            warn!(ctx.system.log(), "Disconnecting peer, because of stalled bootstrap pipeline";
                "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string(),
            );

            // TODO: unsubscribe from channel ?
            // TODO: plus pridat stop IF na spracovanie akcii
            ctx.system.stop(ctx.myself());
            ctx.system.stop(self.peer.peer_ref.clone());
        }
    }
}

fn block_metadata(
    block_hash: &BlockHash,
    block_meta_storage: &BlockMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
) -> Result<Option<(InnerBlockState, Arc<BlockHash>)>, StateError> {
    Ok(match block_meta_storage.get(block_hash)? {
        Some(metadata) => match metadata.predecessor() {
            Some(predecessor) => Some((
                InnerBlockState {
                    block_downloaded: metadata.is_downloaded(),
                    applied: metadata.is_applied(),
                    operations_downloaded: operations_meta_storage.is_complete(block_hash)?,
                },
                Arc::new(predecessor.clone()),
            )),
            None => None,
        },
        None => None,
    })
}

fn schedule_block_downloading(
    peer: &Arc<PeerId>,
    peer_queues: &DataQueues,
    bootstrap: &mut BootstrapState,
    requester: &DataRequesterRef,
    block_meta_storage: &BlockMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
    max_bootstrap_interval_look_ahead_count: i8,
    log: &Logger,
) -> bool {
    // get peer's actual queued items and available capacity for operations
    let (already_queued, available_queue_capacity) = match peer_queues
        .get_already_queued_block_headers_and_max_capacity()
    {
        Ok(queued_and_capacity) => queued_and_capacity,
        Err(e) => {
            warn!(log, "Failed to get available blocks queue capacity for peer (so ignore this step run)"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            return false;
        }
    };

    // get blocks to download
    match bootstrap.find_next_blocks_to_download(
        available_queue_capacity,
        already_queued,
        max_bootstrap_interval_look_ahead_count,
        |block_hash| block_metadata(block_hash, block_meta_storage, operations_meta_storage),
    ) {
        Ok(blocks_to_download) => {
            // try schedule
            match requester.fetch_block_headers(blocks_to_download, peer, peer_queues, log) {
                Ok(was_scheduled) => was_scheduled,
                Err(e) => {
                    warn!(log, "Failed to schedule blocks for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
                    false
                }
            }
        }
        Err(e) => {
            warn!(log, "Failed to find blocks for scheduling for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            false
        }
    }
}

fn schedule_operations_downloading(
    peer: &mut Arc<PeerId>,
    peer_queues: &DataQueues,
    bootstrap: &mut BootstrapState,
    requester: &DataRequesterRef,
    operations_meta_storage: &mut OperationsMetaStorage,
    max_bootstrap_interval_look_ahead_count: i8,
    log: &Logger,
) -> bool {
    // get peer's actual queued items and available capacity for operations
    let (already_queued, available_queue_capacity) = match peer_queues
        .get_already_queued_block_operations_and_max_capacity()
    {
        Ok(queued_and_capacity) => queued_and_capacity,
        Err(e) => {
            warn!(log, "Failed to get available operations queue capacity for peer (so ignore this step run)"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            return false;
        }
    };

    // get blocks to download
    match bootstrap.find_next_block_operations_to_download(
        available_queue_capacity,
        already_queued,
        max_bootstrap_interval_look_ahead_count,
        |block_hash| {
            operations_meta_storage
                .is_complete(block_hash)
                .map_err(StateError::from)
        },
    ) {
        Ok(blocks_to_download) => {
            // try schedule
            match requester.fetch_block_operations(blocks_to_download, peer, peer_queues, log) {
                Ok(was_scheduled) => was_scheduled,
                Err(e) => {
                    warn!(log, "Failed to schedule blocks for missing operations for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
                    false
                }
            }
        }
        Err(e) => {
            warn!(log, "Failed to find blocks with missing operations for scheduling for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            false
        }
    }
}

fn schedule_block_applying(
    peer: &mut Arc<PeerId>,
    bootstrap: &mut BootstrapState,
    queued_block_headers_for_apply: &mut HashMap<Arc<BlockHash>, Instant>,
    block_meta_storage: &mut BlockMetaStorage,
    requester: &DataRequesterRef,
    myself: PeerBranchBootstrapperRef,
    log: &Logger,
) {
    // find next block to apply
    match bootstrap.find_next_block_to_apply(|block_hash| {
        block_meta_storage
            .is_applied(block_hash)
            .map_err(StateError::from)
    }) {
        Ok(Some(block_to_apply)) => {
            // try schedule
            match requester.try_schedule_apply_block(
                block_to_apply,
                bootstrap.chain_id().clone(),
                None,
                Some(myself),
                Some(queued_block_headers_for_apply),
            ) {
                Ok(_) => (),
                Err(e) => {
                    warn!(log, "Failed to schedule blocks for apply for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
                }
            }
        }
        Ok(None) => {
            // nothing to schedule, so just doing nothing
        }
        Err(e) => {
            warn!(log, "Failed to find non-applied block for scheduling for peer"; "reason" => e,
                        "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
        }
    }
}
