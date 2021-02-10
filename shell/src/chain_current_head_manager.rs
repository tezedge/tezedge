// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Manages current_head shared attribute for chain_manager actor.
//! Main purpouse is to just handle new current head validated by chain_feeder.
//! This is optimization and save time for chain_manager to do other work, like p2p communication and mempool.
//!
//! Responsible for:
//! -- managing attribute current head
//! -- ...

use std::sync::Arc;
use std::time::Instant;

use riker::actors::*;
use slog::{debug, info, warn};

use crypto::hash::{BlockHash, ChainId};
use storage::persistent::PersistentStorage;
use storage::{BlockStorage, BlockStorageReader, StorageInitInfo};

use crate::mempool::CurrentMempoolStateStorageRef;
use crate::shell_channel::{ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::state::head_state::{CurrentHeadRef, HeadResult, HeadState};
use crate::state::synchronization_state::SynchronizationBootstrapStateRef;
use crate::state::StateError;
use crate::stats::apply_block_stats::{ApplyBlockStatsRef, BlockValidationTimer};

/// Message commands [`ChainCurrentHeadManager`] to process applied block.
/// Chain_feeder propagates if block successfully validated and applied
/// This is not the same as NewCurrentHead, not every applied block is set as NewCurrentHead (reorg - several headers on same level, duplicate header ...)
#[derive(Clone, Debug)]
pub struct ProcessValidatedBlock {
    block: Arc<BlockHash>,
    chain_id: Arc<ChainId>,

    roundtrip_timer: Arc<Instant>,
    validation_timer: Arc<BlockValidationTimer>,
}

impl ProcessValidatedBlock {
    pub fn new(
        block: Arc<BlockHash>,
        chain_id: Arc<ChainId>,
        roundtrip_timer: Arc<Instant>,
        validation_timer: Arc<BlockValidationTimer>,
    ) -> Self {
        Self {
            block,
            chain_id,
            roundtrip_timer,
            validation_timer,
        }
    }
}

/// Purpose of this actor is to perform chain synchronization.
#[actor(ProcessValidatedBlock)]
pub struct ChainCurrentHeadManager {
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,

    /// Block storage
    block_storage: Box<dyn BlockStorageReader>,

    /// Helps to manage current head
    head_state: HeadState,
    /// Holds bootstrapped state
    current_bootstrap_state: SynchronizationBootstrapStateRef,
    /// Holds "best" known remote head
    remote_current_head_state: CurrentHeadRef,

    /// Internal stats
    apply_block_stats: ApplyBlockStatsRef,
}

/// Reference to [chain manager](ChainManager) actor.
pub type ChainCurrentHeadManagerRef = ActorRef<ChainCurrentHeadManagerMsg>;

impl ChainCurrentHeadManager {
    /// Create new actor instance.
    pub fn actor(
        sys: &ActorSystem,
        shell_channel: ShellChannelRef,
        persistent_storage: PersistentStorage,
        init_storage_data: StorageInitInfo,
        local_current_head_state: CurrentHeadRef,
        remote_current_head_state: CurrentHeadRef,
        current_mempool_state: CurrentMempoolStateStorageRef,
        current_bootstrap_state: SynchronizationBootstrapStateRef,
        apply_block_stats: ApplyBlockStatsRef,
    ) -> Result<ChainCurrentHeadManagerRef, CreateError> {
        sys.actor_of_props::<ChainCurrentHeadManager>(
            ChainCurrentHeadManager::name(),
            Props::new_args((
                shell_channel,
                persistent_storage,
                init_storage_data,
                local_current_head_state,
                remote_current_head_state,
                current_mempool_state,
                current_bootstrap_state,
                apply_block_stats,
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
        let ProcessValidatedBlock {
            block,
            chain_id,
            roundtrip_timer,
            validation_timer,
        } = validated_block;

        // TODO: TE-369 - check if just block metadata with fitness is not enought?

        // read block
        let block = match self.block_storage.get(&block)? {
            Some(block) => Arc::new(block),
            None => {
                return Err(StateError::ProcessingError {
                    reason: format!(
                        "Block/json_data not found for block_hash: {}",
                        block.to_base58_check()
                    ),
                });
            }
        };

        // we try to set it as "new current head", if some means set, if none means just ignore block
        if let Some((new_head, new_head_result)) =
            self.head_state.try_update_new_current_head(&block)?
        {
            debug!(ctx.system.log(), "New current head";
                                     "block_header_hash" => new_head.block_hash().to_base58_check(),
                                     "level" => new_head.level(),
                                     "result" => format!("{}", new_head_result)
            );

            // notify other actors that new current head was changed
            // (this also notifies [mempool_prevalidator])
            self.shell_channel.tell(
                Publish {
                    msg: ShellChannelMsg::NewCurrentHead(new_head.clone(), block.clone()),
                    topic: ShellChannelTopic::ShellNewCurrentHead.into(),
                },
                None,
            );

            let mut is_bootstrapped = self.current_bootstrap_state.read()?.is_bootstrapped();

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
                match new_head_result {
                    HeadResult::BranchSwitch => {
                        self.shell_channel.tell(
                            Publish {
                                msg: ShellChannelMsg::AdvertiseToP2pNewCurrentBranch(
                                    chain_id,
                                    Arc::new(new_head.block_hash().clone()),
                                ),
                                topic: ShellChannelTopic::ShellCommands.into(),
                            },
                            None,
                        );
                    }
                    HeadResult::HeadIncrement => {
                        self.shell_channel.tell(
                            Publish {
                                msg: ShellChannelMsg::AdvertiseToP2pNewCurrentHead(
                                    chain_id,
                                    Arc::new(new_head.block_hash().clone()),
                                ),
                                topic: ShellChannelTopic::ShellCommands.into(),
                            },
                            None,
                        );
                    }
                    HeadResult::GenesisInitialized => {
                        (/* doing nothing, we dont advertise genesis */)
                    }
                }
            }

            // update internal state
            let mut apply_block_stats = self.apply_block_stats.write()?;
            apply_block_stats.set_applied_block_level(*new_head.level());
        }

        // add to stats
        self.apply_block_stats
            .write()?
            .add_block_validation_stats(roundtrip_timer, validation_timer);

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
                warn!(ctx.system.log(), "Failed to collect local head debug info"; "reason" => format!("{}", e));
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
}

impl
    ActorFactoryArgs<(
        ShellChannelRef,
        PersistentStorage,
        StorageInitInfo,
        CurrentHeadRef,
        CurrentHeadRef,
        CurrentMempoolStateStorageRef,
        SynchronizationBootstrapStateRef,
        ApplyBlockStatsRef,
    )> for ChainCurrentHeadManager
{
    fn create_args(
        (
            shell_channel,
            persistent_storage,
            init_storage_data,
            local_current_head_state,
            remote_current_head_state,
            current_mempool_state,
            current_bootstrap_state,
            apply_block_stats,
        ): (
            ShellChannelRef,
            PersistentStorage,
            StorageInitInfo,
            CurrentHeadRef,
            CurrentHeadRef,
            CurrentMempoolStateStorageRef,
            SynchronizationBootstrapStateRef,
            ApplyBlockStatsRef,
        ),
    ) -> Self {
        ChainCurrentHeadManager {
            shell_channel,
            block_storage: Box::new(BlockStorage::new(&persistent_storage)),
            head_state: HeadState::new(
                &persistent_storage,
                local_current_head_state,
                current_mempool_state,
                Arc::new(init_storage_data.chain_id),
                Arc::new(init_storage_data.genesis_block_header_hash),
            ),
            current_bootstrap_state,
            remote_current_head_state,
            apply_block_stats,
        }
    }
}

impl Actor for ChainCurrentHeadManager {
    type Msg = ChainCurrentHeadManagerMsg;

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        // now we can hydrate state and read current head
        self.hydrate_current_head_state(ctx);
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
