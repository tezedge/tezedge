// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

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
use crate::utils::DeadlineTryLockGuard;

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

/// Event is fired, when some batch was finished, so next can go
#[derive(Clone, Debug)]
pub struct BlockBatchApplied {
    pub last_applied: Arc<BlockHash>,
}

/// Event is fired, when some batch was not applied and error occured
#[derive(Clone, Debug)]
pub struct BlockBatchApplyFailed {
    pub last_applied: Option<Arc<BlockHash>>,
}

#[actor(
    StartBranchBootstraping,
    PingBootstrapPipelinesProcessing,
    UpdateBlockState,
    UpdateOperationsState,
    BlockBatchApplied,
    BlockBatchApplyFailed,
    DisconnectStalledBootstraps,
    SystemEvent
)]
pub struct PeerBranchBootstrapper {
    peer: Arc<PeerId>,
    peer_queues: Arc<DataQueues>,
    block_batch_apply_try_lock: Option<DeadlineTryLockGuard>,

    shell_channel: ShellChannelRef,
    block_meta_storage: BlockMetaStorage,
    operations_meta_storage: OperationsMetaStorage,

    requester: DataRequesterRef,

    bootstrap_state: Vec<BootstrapState>,
    empty_bootstrap_state: Option<Instant>,

    cfg: PeerBranchBootstrapperConfiguration,

    /// Indicates that we already scheduled process_bootstrap_pipelines, means we have already unprocessed message in mailbox
    /// So, we dont need to add it twice - this is kind if optimization, not to exhause mailbox with the same messages
    process_bootstrap_pipelines_triggered: AtomicBool,
}

#[derive(Clone)]
pub struct PeerBranchBootstrapperConfiguration {
    max_bootstrap_interval_look_ahead_count: i8,
    max_bootstrap_branches_per_peer: usize,
    max_block_apply_batch: usize,
}

impl PeerBranchBootstrapperConfiguration {
    pub fn new(
        max_bootstrap_interval_look_ahead_count: i8,
        max_bootstrap_branches_per_peer: usize,
        max_block_apply_batch: usize,
    ) -> Self {
        Self {
            max_bootstrap_interval_look_ahead_count,
            max_bootstrap_branches_per_peer,
            max_block_apply_batch,
        }
    }
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
        cfg: PeerBranchBootstrapperConfiguration,
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
                cfg,
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

        // next check apply blocks
        self.process_block_apply(ctx.myself(), log);

        // check, if we completed any pipeline
        self.handle_resolved_bootstraps(log);

        // check if we need to release lock
        self.check_deadline_for_block_batch_apply_try_lock(log);
    }

    fn check_deadline_for_block_batch_apply_try_lock(&mut self, log: &Logger) {
        if let Some(lock) = self.block_batch_apply_try_lock.as_ref() {
            if lock.is_deadline_reached() {
                self.release_block_batch_apply_try_lock(log);
            }
        }
    }

    fn release_block_batch_apply_try_lock(&mut self, log: &Logger) {
        if let Some(lock) = self.block_batch_apply_try_lock.take() {
            drop(lock);
        }
    }

    fn process_data_download(&mut self, log: &Logger) -> bool {
        let PeerBranchBootstrapper {
            peer,
            peer_queues,
            bootstrap_state,
            requester,
            block_meta_storage,
            operations_meta_storage,
            cfg,
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
                cfg.max_bootstrap_interval_look_ahead_count,
                log,
            );

            // schedule next block downloading
            was_scheduled |= schedule_operations_downloading(
                peer,
                peer_queues,
                bootstrap,
                requester,
                operations_meta_storage,
                cfg.max_bootstrap_interval_look_ahead_count,
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
            requester,
            cfg,
            block_batch_apply_try_lock: block_apply_try_lock,
            ..
        } = self;

        bootstrap_state.iter_mut().for_each(|bootstrap| {
            // schedule next block for apply
            schedule_block_applying(
                peer,
                bootstrap,
                block_apply_try_lock,
                block_meta_storage,
                requester,
                myself.clone(),
                cfg.max_block_apply_batch,
                log,
            );
        });
    }

    fn handle_resolved_bootstraps(&mut self, log: &Logger) {
        let PeerBranchBootstrapper {
            bootstrap_state,
            peer,
            shell_channel,
            ..
        } = self;

        bootstrap_state.retain(|b| {
            if b.is_done() {
                debug!(log, "Finished branch bootstrapping process";
                                         "to_level" => b.to_level(),
                                         "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());

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
        PeerBranchBootstrapperConfiguration,
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
            cfg,
        ): (
            Arc<PeerId>,
            Arc<DataQueues>,
            DataRequesterRef,
            ShellChannelRef,
            BlockMetaStorage,
            OperationsMetaStorage,
            PeerBranchBootstrapperConfiguration,
        ),
    ) -> Self {
        PeerBranchBootstrapper {
            peer,
            peer_queues,
            requester,
            block_batch_apply_try_lock: None,
            bootstrap_state: Default::default(),
            shell_channel,
            block_meta_storage,
            operations_meta_storage,
            empty_bootstrap_state: None,
            process_bootstrap_pipelines_triggered: AtomicBool::new(false),
            cfg,
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
        let log = ctx.system.log();
        debug!(log, "Start branch bootstrapping process";
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
        self.handle_resolved_bootstraps(&log);

        if self.bootstrap_state.len() >= self.cfg.max_bootstrap_branches_per_peer {
            debug!(log, "Peer has started already maximum ({}) pipeline, so we dont start new one", self.cfg.max_bootstrap_branches_per_peer;
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
        self.process_bootstrap_pipelines(ctx, &log)
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

impl Receive<BlockBatchApplied> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: BlockBatchApplied,
        _: Option<BasicActorRef>,
    ) {
        // release lock
        self.release_block_batch_apply_try_lock(&ctx.system.log());

        // process message
        self.bootstrap_state.iter_mut().for_each(|bootstrap| {
            bootstrap.block_applied(&msg.last_applied);
        });

        // process
        self.process_bootstrap_pipelines(ctx, &ctx.system.log())
    }
}

impl Receive<BlockBatchApplyFailed> for PeerBranchBootstrapper {
    type Msg = PeerBranchBootstrapperMsg;

    fn receive(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: BlockBatchApplyFailed,
        _: Option<BasicActorRef>,
    ) {
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
        let log = ctx.system.log();
        self.handle_resolved_bootstraps(&log);

        // check for any stalled bootstrap
        let mut is_stalled = self
            .bootstrap_state
            .iter()
            .any(|bootstrap| bootstrap.is_stalled(msg.timeout));

        if let Some(empty_bootstrap_state) = self.empty_bootstrap_state {
            if empty_bootstrap_state.elapsed() > MISSING_NEW_BOOTSTRAP_TIMEOUT {
                warn!(log, "Peer did not sent new curent_head/current_branch for a long time";
                    "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string(),
                );
                is_stalled = true;
            }
        }

        // if stalled, just disconnect peer
        if is_stalled {
            warn!(log, "Disconnecting peer, because of stalled bootstrap pipeline";
                "peer_id" => self.peer.peer_id_marker.clone(), "peer_ip" => self.peer.peer_address.to_string(), "peer" => self.peer.peer_ref.name(), "peer_uri" => self.peer.peer_ref.uri().to_string(),
            );

            // stop actors for peer
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
    block_apply_try_lock: &mut Option<DeadlineTryLockGuard>,
    block_meta_storage: &mut BlockMetaStorage,
    requester: &DataRequesterRef,
    myself: PeerBranchBootstrapperRef,
    max_block_apply_batch: usize,
    log: &Logger,
) {
    // if we hold the lock, means we are waiting for our scheduled batch to finish
    if block_apply_try_lock.is_some() {
        return;
    }

    // find next block to apply (and mark existing blocks as applied)
    match bootstrap.find_next_block_to_apply(max_block_apply_batch, |block_hash| {
        block_meta_storage
            .is_applied(block_hash)
            .map_err(StateError::from)
    }) {
        Ok(Some(batch)) => {
            // try schedule
            match requester.try_schedule_apply_block(
                bootstrap.chain_id().clone(),
                batch,
                Some(myself),
            ) {
                Ok(Some(lock)) => {
                    *block_apply_try_lock = Some(lock);
                    ()
                }
                Ok(None) => {
                    // no lock acquired, so not our turn
                    ()
                }
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
