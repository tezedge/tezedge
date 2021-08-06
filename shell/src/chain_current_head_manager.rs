// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Manages current_head shared attribute for chain_manager actor.
//! Main purpouse is to just handle new current head validated by chain_feeder.
//! This is optimization and save time for chain_manager to do other work, like p2p communication and mempool.
//!
//! Responsible for:
//! -- managing attribute current head
//! -- ...

use std::sync::Arc;

use riker::actors::*;
use slog::{debug, info, warn, Logger};

use crypto::hash::ChainId;
use storage::StorageInitInfo;
use storage::{BlockHeaderWithHash, PersistentStorage};

use crate::mempool::mempool_prevalidator::{
    MempoolPrevalidatorBasicRef, MempoolPrevalidatorMsg, ResetMempool,
};
use crate::mempool::{CurrentMempoolStateStorageRef, MempoolPrevalidatorFactory};
use crate::state::head_state::{CurrentHeadRef, HeadResult, HeadState};
use crate::state::synchronization_state::SynchronizationBootstrapStateRef;
use crate::state::StateError;
use crate::subscription::*;
use crate::tezedge_state_manager::proposer_messages::ProposerMsg;
use crate::tezedge_state_manager::ProposerHandle;

/// Message commands [`ChainCurrentHeadManager`] to process applied block.
/// Chain_feeder propagates if block successfully validated and applied
/// This is not the same as NewCurrentHead, not every applied block is set as NewCurrentHead (reorg - several headers on same level, duplicate header ...)
#[derive(Clone, Debug)]
pub struct ProcessValidatedBlock {
    pub block: Arc<BlockHeaderWithHash>,
    chain_id: Arc<ChainId>,
}

impl ProcessValidatedBlock {
    pub fn new(block: Arc<BlockHeaderWithHash>, chain_id: Arc<ChainId>) -> Self {
        Self { block, chain_id }
    }
}

/// Purpose of this actor is to perform chain synchronization.
#[actor(ProcessValidatedBlock)]
pub struct ChainCurrentHeadManager {
    proposer: ProposerHandle,
    new_current_head_notifier: Arc<Notifier<NewCurrentHeadNotificationRef>>,

    /// Helps to manage current head
    head_state: HeadState,
    /// Holds bootstrapped state
    current_bootstrap_state: SynchronizationBootstrapStateRef,
    /// Holds "best" known remote head
    remote_current_head_state: CurrentHeadRef,

    /// Mempool prevalidator
    mempool_prevalidator: Option<MempoolPrevalidatorBasicRef>,
    /// mempool factory
    mempool_prevalidator_factory: Arc<MempoolPrevalidatorFactory>,
}

/// Reference to [chain manager](ChainManager) actor.
pub type ChainCurrentHeadManagerRef = ActorRef<ChainCurrentHeadManagerMsg>;

impl ChainCurrentHeadManager {
    /// Create new actor instance.
    pub fn actor(
        sys: &ActorSystem,
        proposer: ProposerHandle,
        new_current_head_notifier: Notifier<NewCurrentHeadNotificationRef>,
        persistent_storage: PersistentStorage,
        init_storage_data: StorageInitInfo,
        local_current_head_state: CurrentHeadRef,
        remote_current_head_state: CurrentHeadRef,
        current_mempool_state: CurrentMempoolStateStorageRef,
        current_bootstrap_state: SynchronizationBootstrapStateRef,
        mempool_prevalidator_factory: Arc<MempoolPrevalidatorFactory>,
    ) -> Result<ChainCurrentHeadManagerRef, CreateError> {
        sys.actor_of_props::<ChainCurrentHeadManager>(
            ChainCurrentHeadManager::name(),
            Props::new_args((
                proposer,
                Arc::new(new_current_head_notifier),
                persistent_storage,
                init_storage_data,
                local_current_head_state,
                remote_current_head_state,
                current_mempool_state,
                current_bootstrap_state,
                mempool_prevalidator_factory,
            )),
        )
    }

    fn name() -> &'static str {
        "chain-current-head-manager"
    }

    /// Handles validated block.
    ///
    /// This logic is equivalent to [chain_validator.ml][let on_request]
    /// if applied block is winner we need to:
    /// - set current head
    /// - set bootstrapped flag
    /// - broadcast new current head/branch to peers (if bootstrapped)
    /// - start test chain (if needed) (TODO: TE-123 - not implemented yet)
    /// - update checkpoint (TODO: TE-210 - not implemented yet)
    /// - reset mempool_prevalidator
    /// ...
    fn process_applied_block(
        &mut self,
        ctx: &Context<ChainCurrentHeadManagerMsg>,
        validated_block: ProcessValidatedBlock,
    ) -> Result<(), StateError> {
        let ProcessValidatedBlock { block, chain_id } = validated_block;

        // we try to set it as "new current head", if some means set, if none means just ignore block
        if let Some((new_head, new_head_result)) =
            self.head_state.try_update_new_current_head(&block)?
        {
            debug!(ctx.system.log(), "New current head";
                                     "block_header_hash" => new_head.block_hash().to_base58_check(),
                                     "level" => new_head.level(),
                                     "result" => format!("{}", new_head_result)
            );

            let mut is_bootstrapped = self.current_bootstrap_state.read()?.is_bootstrapped();

            // notify registered subscribers that new current head was changed
            self.new_current_head_notifier
                .notify(Arc::new(NewCurrentHeadNotification::new(
                    chain_id.clone(),
                    block.clone(),
                    is_bootstrapped,
                )));

            if !is_bootstrapped {
                let chain_manager_current_level = new_head.level();

                let remote_best_known_level = self
                    .remote_current_head_state
                    .read()?
                    .as_ref()
                    .map(|head| *head.level())
                    .unwrap_or(0);

                let mut current_bootstrap_state = self.current_bootstrap_state.write()?;
                is_bootstrapped = current_bootstrap_state.update_by_new_local_head(
                    remote_best_known_level,
                    *chain_manager_current_level,
                );

                if is_bootstrapped {
                    info!(ctx.system.log(), "Bootstrapped (chain_current_head_manager)";
                       "num_of_peers_for_bootstrap_threshold" => current_bootstrap_state.num_of_peers_for_bootstrap_threshold(),
                       "remote_best_known_level" => remote_best_known_level,
                       "reached_on_level" => chain_manager_current_level);
                }
            }

            // TODO: TE-369 - lazy feature, if multiple messages are waiting in queue, we just want to send the last one as first one and the other discard

            // broadcast new head/branch to other peers
            // we can do this, only if we are bootstrapped,
            // e.g. if we just start to bootstrap from the scratch, we dont want to spam other nodes (with higher level)
            if is_bootstrapped {
                // notify mempool if enabled
                if !self.mempool_prevalidator_factory.p2p_disable_mempool {
                    // find prevalidator for chain_id, if not found, then stop
                    match self.mempool_if_allowed(&chain_id, &ctx.system, &ctx.system.log()) {
                        Ok(Some(mempool_prevalidator)) => {
                            // ping mempool to reset head
                            if mempool_prevalidator
                                .try_tell(
                                    MempoolPrevalidatorMsg::ResetMempool(ResetMempool { block }),
                                    None,
                                )
                                .is_err()
                            {
                                warn!(ctx.system.log(), "Reset mempool error, mempool_prevalidator does not support message `ResetMempool`!"; "caller" => "chain_current_head_manager");
                            }
                        }
                        Ok(None) => {
                            warn!(ctx.system.log(), "No mempool prevalidator was found, so cannot reset mempool";
                                                    "chain_id" => chain_id.to_base58_check(),
                                                    "block_hash" => new_head.block_hash().to_base58_check(),
                                                    "caller" => "chain_current_head_manager");
                        }
                        Err(err) => {
                            warn!(ctx.system.log(), "Failed to instantiate mempool_prevalidator";
                                                    "chain_id" => chain_id.to_base58_check(),
                                                    "block_hash" => new_head.block_hash().to_base58_check(),
                                                    "caller" => "chain_current_head_manager",
                                                    "reason" => err);
                        }
                    }
                }

                // advertise new branch or new head
                match new_head_result {
                    HeadResult::BranchSwitch => {
                        if let Err(err) =
                            self.proposer
                                .notify(ProposerMsg::AdvertiseToP2pNewCurrentBranch(
                                    chain_id,
                                    Arc::new(new_head.block_hash().clone()),
                                ))
                        {
                            warn!(ctx.system.log(), "Failed to notify proposer (AdvertiseToP2pNewCurrentBranch)"; "reason" => format!("{:?}", err));
                        }
                    }
                    HeadResult::HeadIncrement => {
                        if let Err(err) =
                            self.proposer
                                .notify(ProposerMsg::AdvertiseToP2pNewCurrentHead(
                                    chain_id,
                                    Arc::new(new_head.block_hash().clone()),
                                ))
                        {
                            warn!(ctx.system.log(), "Failed to notify proposer (AdvertiseToP2pNewCurrentHead)"; "reason" => format!("{:?}", err));
                        }
                    }
                    HeadResult::GenesisInitialized => {
                        (/* doing nothing, we dont advertise genesis */)
                    }
                }
            }
        }

        Ok(())
    }

    fn hydrate_current_head_state(&mut self, ctx: &Context<ChainCurrentHeadManagerMsg>) {
        info!(ctx.system.log(), "Hydrating/loading current head");
        let (local_head, local_head_level, local_fitness) = match self
            .head_state
            .load_current_head_state()
        {
            Ok(head) => match head {
                None => ("-none-".to_string(), 0_i32, "-none-".to_string()),
                Some(head) => head.to_debug_info(),
            },
            Err(e) => {
                warn!(ctx.system.log(), "Failed to collect local head debug info"; "reason" => e);
                (
                    "-failed-to-collect-".to_string(),
                    0,
                    "-failed-to-collect-".to_string(),
                )
            }
        };
        info!(
            ctx.system.log(),
            "Hydrating current_head completed successfully";
            "local_head" => local_head,
            "local_head_level" => local_head_level,
            "local_fitness" => local_fitness,
        );
    }

    fn mempool_if_allowed(
        &mut self,
        chain_id: &ChainId,
        sys: &ActorSystem,
        log: &Logger,
    ) -> Result<Option<&MempoolPrevalidatorBasicRef>, StateError> {
        if self.mempool_prevalidator.is_none() {
            self.mempool_prevalidator = self.mempool_prevalidator_factory.get_or_start_mempool(
                chain_id.clone(),
                sys,
                log,
            )?;
        }
        Ok(self.mempool_prevalidator.as_ref())
    }
}

impl
    ActorFactoryArgs<(
        ProposerHandle,
        Arc<Notifier<NewCurrentHeadNotificationRef>>,
        PersistentStorage,
        StorageInitInfo,
        CurrentHeadRef,
        CurrentHeadRef,
        CurrentMempoolStateStorageRef,
        SynchronizationBootstrapStateRef,
        Arc<MempoolPrevalidatorFactory>,
    )> for ChainCurrentHeadManager
{
    fn create_args(
        (
            proposer,
            new_current_head_notifier,
            persistent_storage,
            init_storage_data,
            local_current_head_state,
            remote_current_head_state,
            current_mempool_state,
            current_bootstrap_state,
            mempool_prevalidator_factory,
        ): (
            ProposerHandle,
            Arc<Notifier<NewCurrentHeadNotificationRef>>,
            PersistentStorage,
            StorageInitInfo,
            CurrentHeadRef,
            CurrentHeadRef,
            CurrentMempoolStateStorageRef,
            SynchronizationBootstrapStateRef,
            Arc<MempoolPrevalidatorFactory>,
        ),
    ) -> Self {
        ChainCurrentHeadManager {
            proposer,
            new_current_head_notifier,
            head_state: HeadState::new(
                &persistent_storage,
                local_current_head_state,
                current_mempool_state,
                Arc::new(init_storage_data.chain_id),
                Arc::new(init_storage_data.genesis_block_header_hash),
            ),
            current_bootstrap_state,
            remote_current_head_state,
            mempool_prevalidator: None,
            mempool_prevalidator_factory,
        }
    }
}

impl Actor for ChainCurrentHeadManager {
    type Msg = ChainCurrentHeadManagerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        // now we can hydrate state and read current head
        self.hydrate_current_head_state(ctx);

        info!(ctx.system.log(), "Chain current head manager started");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ProcessValidatedBlock> for ChainCurrentHeadManager {
    type Msg = ChainCurrentHeadManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ProcessValidatedBlock, _: Sender) {
        match self.process_applied_block(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to process validated block"; "reason" => format!("{:?}", e))
            }
        }
    }
}
