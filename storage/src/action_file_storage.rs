use crate::action_file::*;
use crate::persistent::{ActionRecordError, ActionRecorder};
use crate::StorageError;
use crypto::hash::BlockHash;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use tezos_context::channel::{ContextAction, ContextActionMessage};

pub struct ActionFileStorage {
    file: PathBuf,
    staging: HashMap<BlockHash, Vec<ContextActionMessage>>,
    level: u32,
}

impl ActionFileStorage {
    pub fn new(path: PathBuf) -> Self {
        ActionFileStorage {
            file: path,
            staging: HashMap::new(),
            level: 0,
        }
    }

    fn store_single_message(&mut self, block_hash: BlockHash, msg: &ContextActionMessage) {
        let block_actions = self.staging.entry(block_hash).or_insert(Vec::new());
        block_actions.push(msg.clone());
    }

    fn store_commit_message(
        &mut self,
        block_hash: BlockHash,
        msg: &ContextActionMessage,
    ) -> Result<(), StorageError> {
        self.level += 1;
        self.store_single_message(block_hash.clone(), msg);
        self.flush_entries_to_file(block_hash)
    }

    fn flush_entries_to_file(&mut self, block_hash: BlockHash) -> Result<(), StorageError> {
        let mut action_file_writer =
            ActionsFileWriter::new(&self.file).map_err(|e| StorageError::ActionRecordError {
                error: ActionRecordError::ActionFileError { error: e },
            })?;

        let actions =
            self.staging
                .remove(&block_hash)
                .ok_or(ActionRecordError::MissingActions {
                    hash: block_hash.to_base58_check(),
                })?;

        action_file_writer
            .update(actions)
            .map_err(ActionRecordError::from)?;
        Ok(())
    }
}

impl ActionRecorder for ActionFileStorage {
    fn record(
        &mut self,
        context_action_message: &ContextActionMessage,
    ) -> std::result::Result<(), StorageError> {
        match &context_action_message.action {
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
                self.store_single_message(
                    BlockHash::try_from(block_hash.clone())?,
                    context_action_message,
                );
                Ok(())
            }
            ContextAction::Commit {
                block_hash: Some(block_hash),
                ..
            } => self.store_commit_message(
                BlockHash::try_from(block_hash.clone())?,
                &context_action_message,
            ),
            _ => Ok(()),
        }
    }
}
