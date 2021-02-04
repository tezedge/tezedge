use crate::persistent::PersistentStorage;
use crate::{BlockStorage, BlockStorageReader};
use crate::action_file::*;
use crate::persistent::{ActionRecorder, ActionRecordError};
use crypto::hash::BlockHash;
use std::collections::HashMap;
use std::path::PathBuf;
use crate::StorageError;
use tezos_context::channel::{ContextAction, ContextActionMessage};

pub struct ActionFileStorage {
    block_storage: BlockStorage,
    file: PathBuf,
    staging: HashMap<Vec<u8>, Vec<ContextActionMessage>>,
}

use slog::{error,warn,Logger};

impl ActionFileStorage {
    pub fn new(path: PathBuf, persistent_storage: &PersistentStorage) -> Self {
        ActionFileStorage {
            file: path,
            staging: HashMap::new(),
            block_storage: BlockStorage::new(persistent_storage),
        }
    }

    fn store_single_message(&mut self, block_hash: &BlockHash, msg: &ContextActionMessage){
        let block_actions = self.staging.entry(block_hash.clone()).or_insert(Vec::new());
        block_actions.push(msg.clone());
    }

    fn store_commit_message(&mut self, block_hash: &BlockHash, msg: &ContextActionMessage) -> Result<(), StorageError>{
        self.store_single_message(block_hash, msg);
        return self.flush_entries_to_file(block_hash);
    }

    fn flush_entries_to_file(&mut self, block_hash: &BlockHash) -> Result<(), StorageError>{
        let mut action_file_writer = ActionsFileWriter::new(&self.file)
            .or_else(|e| 
                Err(
                    StorageError::ActionRecordError{
                        error: ActionRecordError::ActionFileError{error: e}
                    }))?;

        // Get block level from Block storage
        match self.block_storage.get(block_hash)?{
            Some(block_header) => {
                let block = 
                    Block::new(
                        block_header.header.level() as u32,
                        block_header.hash,
                        block_header.header.predecessor().to_vec(),
                    );    

                // remove block actions from staging and save it to action file
                let actions = self.staging.remove(block_hash).ok_or(
                    ActionRecordError::MissingActions{hash : hex::encode(&block_hash)}
                )?;

                action_file_writer.update(block, actions).or_else(
                    |e| Err(ActionRecordError::from(e))
                )?;
                Ok(())
            }
            None => {Ok(())}
        }

    }

}

impl ActionRecorder for ActionFileStorage {
    fn record(&mut self, context_action_message: &ContextActionMessage) -> std::result::Result<(), StorageError> {
        match &context_action_message.action {
            ContextAction::Set { block_hash: Some(block_hash), ..}
            | ContextAction::Copy { block_hash: Some(block_hash), ..  }
            | ContextAction::Delete { block_hash: Some(block_hash), ..  }
            | ContextAction::RemoveRecursively { block_hash: Some(block_hash), ..  }
            | ContextAction::Mem { block_hash: Some(block_hash), ..  }
            | ContextAction::DirMem { block_hash: Some(block_hash), ..  }
            | ContextAction::Get { block_hash: Some(block_hash), ..  }
            | ContextAction::Fold { block_hash: Some(block_hash), ..  } => {
                self.store_single_message(block_hash, context_action_message);
                Ok(())
            }
            ContextAction::Commit { block_hash: Some(block_hash), .. } => {
                self.store_commit_message(&block_hash, &context_action_message)
            }
            _ => {Ok(())}
        }
    }
}
