use tezos_context::channel::ContextAction;
use dashmap::DashMap;
use std::sync::Arc;
use action_sync::*;
use crate::{BlockStorage, BlockStorageReader, BlockHeaderWithHash, StorageError, BlockJsonData};
use crate::persistent::PersistentStorage;

pub struct ActionFileStorage {
    block_storage : BlockStorage,
    file: String,
    staging: Arc<DashMap<Vec<u8>, Vec<ContextAction>>>,
}

///staging: Arc<DashMap<String, Vec<ContextAction>>>

impl ActionFileStorage {
    pub fn new(file: String, persistence : &PersistentStorage ) -> ActionFileStorage {
        ActionFileStorage {
            file,
            staging : persistence.actions_staging(),
            block_storage : BlockStorage::new(persistence)
        }
    }
}

impl ActionFileStorage {
    fn set_in_staging(&mut self, action : ContextAction) {
        match &action {
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

                let mut block_actions = self.staging.entry(block_hash.clone()).or_insert(Vec::new());
                block_actions.push(action);


            }
            ContextAction::Commit { block_hash,..} => {
                let block_hash = match block_hash {
                    None => {
                        return;
                    }
                    Some(h) => {
                        h
                    }
                };

                let mut block_actions = self.staging.entry(block_hash.clone()).or_insert(Vec::new());
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
                let block = match self.block_storage.get_with_json_data(block_hash) {
                    Ok(b) => {
                        match b {
                            None => {
                                return;
                            }
                            Some((b,d)) => {

                                Block::new(b.header.level(),
                                           hex::encode(b.hash),
                                           b.header.predecessor()
                                )
                            }
                        }
                    }
                    Err(_) => {
                        return;
                    }
                };

                // remove block action from staging and save it to action file

               if  let Some((_,actions)) = self.staging.remove(block_hash) {
                   action_file_writer.update(block,actions)
               }

            }
            _ => {}
        };
    }
}