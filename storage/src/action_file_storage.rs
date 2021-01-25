use tezos_context::channel::ContextAction;
use std::sync::{Arc, RwLock, PoisonError, RwLockWriteGuard};
use action_sync::*;
use crate::{BlockStorage, BlockStorageReader, BlockHeaderWithHash, StorageError, BlockJsonData};
use crate::persistent::PersistentStorage;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use slog::{error};

pub struct ActionFileStorage {
    block_storage: BlockStorage,
    file: PathBuf,
    staging: Arc<RwLock<HashMap<Vec<u8>, Vec<ContextAction>>>>,
}

///staging: Arc<DashMap<String, Vec<ContextAction>>>

use slog::{crit, debug, info, warn, Logger};

impl ActionFileStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Option<ActionFileStorage> {
        match persistent_storage.action_file_path() {
            None => {
                None
            }
            Some(path) => {
                Some(ActionFileStorage {
                    file: path,
                    staging: persistent_storage.actions_staging(),
                    block_storage: BlockStorage::new(persistent_storage),
                })
            }
        }
    }
}

impl ActionFileStorage {
    pub fn store_action(&mut self, log: &Logger, action: ContextAction) {
        match action.clone() {
            ContextAction::Set {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::Copy {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::Delete {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::RemoveRecursively {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::Mem {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::DirMem {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::Get {
                block_hash: Some(block_hash),
                ..
            }
            | ContextAction::Fold {
                block_hash: Some(block_hash),
                ..
            } => {
                let mut w = match self.staging.write() {
                    Ok(w) => { w }
                    Err(_) => { return; }
                };
                let mut block_actions = w.entry(block_hash.clone()).or_insert(Vec::new());
                block_actions.push(action);
            }
            ContextAction::Commit { block_hash, .. } => {
                let block_hash = match block_hash {
                    None => {
                        return;
                    }
                    Some(h) => {
                        h
                    }
                };
                let mut w = match self.staging.write() {
                    Ok(w) => { w }
                    Err(_) => { return; }
                };
                let mut block_actions = w.entry(block_hash.clone()).or_insert(Vec::new());
                //Todo Check if empty
                block_actions.push(action);

                let mut action_file_writer = match ActionsFileWriter::new(&self.file) {
                    Ok(w) => {
                        w
                    }
                    Err(_) => {
                        return;
                    }
                };

                // Get block level from Block storage
                let block = match self.block_storage.get(&block_hash) {
                    Ok(b) => {
                        match b {
                            None => {
                                return;
                            }
                            Some(b) => {
                                Block::new(b.header.level() as u32, b.hash, b.header.predecessor().to_vec())
                            }
                        }
                    }
                    Err(_) => {
                        return;
                    }
                };

                // remove block action from staging and save it to action file


                if let Some(actions) = w.remove(&block_hash) {
                    let pred = hex::encode(&block.predecessor);
                    let hash = block.block_hash_hex.to_owned();
                    info!(log, "Actions File header {}", action_file_writer.header());
                    info!(log,"Saving Block {}", hash; "predecessor" => pred );
                    match action_file_writer.update(block, actions) {
                        Ok(_) => {
                            info!(log, "Block saved to file {}", hash);
                        }
                        Err(e) => {
                            debug!(log, "Error storing Block {}", hash);
                            error!(log, "Error storing Block {}", e);
                        }
                    };
                }
            }
            _ => {}
        };
    }
}