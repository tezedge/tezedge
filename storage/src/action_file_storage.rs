// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::action_file::*;
use crate::persistent::{ActionRecordError, ActionRecorder};
use crate::StorageError;
use crypto::hash::BlockHash;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;
use tezos_context::channel::ContextAction;

pub struct ActionFileStorage {
    file: PathBuf,
    staging: Vec<ContextAction>,
    level: u32,
}

impl ActionFileStorage {
    pub fn new(path: PathBuf) -> Self {
        ActionFileStorage {
            file: path,
            staging: Vec::new(),
            level: 0,
        }
    }

    fn store_single_action(&mut self, action: &ContextAction) {
        self.staging.push(action.clone());
    }

    fn store_commit_action(
        &mut self,
        action: &ContextAction,
    ) -> Result<(), StorageError> {
        self.level += 1;
        self.store_single_action(action);
        self.flush_entries_to_file()
    }

    fn flush_entries_to_file(&mut self) -> Result<(), StorageError> {
        let mut action_file_writer =
            ActionsFileWriter::new(&self.file).map_err(|e| StorageError::ActionRecordError {
                error: ActionRecordError::ActionFileError { error: e },
            })?;

        action_file_writer
            .update(self.staging.clone())
            .map_err(ActionRecordError::from)?;
        self.staging.clear();
        Ok(())
    }
}

pub fn get_tree_action(action: &ContextAction) -> String {
    match action {
        ContextAction::Get{..} => { "ContextAction::Get".to_string() },
        ContextAction::Mem{..} => { "ContextAction::Mem".to_string() },
        ContextAction::DirMem{..} => { "ContextAction::DirMem".to_string() },
        ContextAction::Set{..} => { "ContextAction::Set".to_string() },
        ContextAction::Copy{..} => { "ContextAction::Copy".to_string() },
        ContextAction::Delete{..} => { "ContextAction::Delete".to_string() },
        ContextAction::RemoveRecursively{..} => { "ContextAction::RemoveRecursively".to_string() },
        ContextAction::Commit{..} => { "ContextAction::Commit".to_string() },
        ContextAction::Fold{..} => { "ContextAction::Fold".to_string() },
        ContextAction::Checkout{..} => { "ContextAction::Checkout".to_string() },
        ContextAction::Shutdown{..} => { "ContextAction::Shutdown".to_string() },
    }
}


impl ActionRecorder for ActionFileStorage {
    fn record(&mut self, context_action: &ContextAction) -> std::result::Result<(), StorageError> {
        match context_action {
            ContextAction::Set {
                ..
            }
            | ContextAction::Copy {
                ..
            }
            | ContextAction::Delete {
                ..
            }
            | ContextAction::RemoveRecursively {
                ..
            }
            | ContextAction::Mem {
                ..
            }
            | ContextAction::DirMem {
                ..
            }
            | ContextAction::Get {
                ..
            }
            | ContextAction::Checkout {
                ..
            }
            | ContextAction::Fold {
                ..
            } => {
                self.store_single_action(context_action);
                Ok(())
            }
            ContextAction::Commit {
                ..
            } => self.store_commit_action(context_action),
            ContextAction::Shutdown => {
                Ok(())
            }
        }
    }
}
