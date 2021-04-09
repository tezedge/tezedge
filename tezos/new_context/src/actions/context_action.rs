// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use std::cmp::Ordering::Equal;

type Hash = Vec<u8>;
type TreeId = i32;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ContextAction {
    Set {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        new_tree_hash: Option<Hash>,
        tree_id: TreeId,
        new_tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
        value: Vec<u8>,
        value_as_json: Option<String>,
    },
    Delete {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        new_tree_hash: Option<Hash>,
        tree_id: TreeId,
        new_tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
    },
    RemoveRecursively {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        new_tree_hash: Option<Hash>,
        tree_id: TreeId,
        new_tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
    },
    Copy {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        new_tree_hash: Option<Hash>,
        tree_id: TreeId,
        new_tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        from_key: Vec<String>,
        to_key: Vec<String>,
    },
    Checkout {
        context_hash: Hash,
        start_time: f64,
        end_time: f64,
    },
    Commit {
        parent_context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        new_context_hash: Hash,
        tree_hash: Option<Hash>,
        tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        author: String,
        message: String,
        date: i64,
        parents: Vec<Vec<u8>>,
    },
    Mem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
        value: bool,
    },
    DirMem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
        value: bool,
    },
    Get {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
        value: Vec<u8>,
        value_as_json: Option<String>,
    },
    Fold {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        tree_hash: Option<Hash>,
        tree_id: TreeId,
        start_time: f64,
        end_time: f64,
        key: Vec<String>,
    },
    /// This is a control event used to shutdown IPC channel
    Shutdown,
}

pub fn get_time(action: &ContextAction) -> f64 {
    match action {
        ContextAction::Set {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Delete {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::RemoveRecursively {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Copy {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Checkout {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Commit {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Mem {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::DirMem {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Get {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Fold {
            start_time,
            end_time,
            ..
        } => *end_time - *start_time,
        ContextAction::Shutdown => 0_f64,
    }
}

pub fn get_end_time(action: &ContextAction) -> f64 {
    match action {
        ContextAction::Set { end_time, .. } => *end_time,
        ContextAction::Delete { end_time, .. } => *end_time,
        ContextAction::RemoveRecursively { end_time, .. } => *end_time,
        ContextAction::Copy { end_time, .. } => *end_time,
        ContextAction::Checkout { end_time, .. } => *end_time,
        ContextAction::Commit { end_time, .. } => *end_time,
        ContextAction::Mem { end_time, .. } => *end_time,
        ContextAction::DirMem { end_time, .. } => *end_time,
        ContextAction::Get { end_time, .. } => *end_time,
        ContextAction::Fold { end_time, .. } => *end_time,
        ContextAction::Shutdown => 0_f64,
    }
}

pub fn get_start_time(action: &ContextAction) -> f64 {
    match action {
        ContextAction::Set { start_time, .. } => *start_time,
        ContextAction::Delete { start_time, .. } => *start_time,
        ContextAction::RemoveRecursively { start_time, .. } => *start_time,
        ContextAction::Copy { start_time, .. } => *start_time,
        ContextAction::Checkout { start_time, .. } => *start_time,
        ContextAction::Commit { start_time, .. } => *start_time,
        ContextAction::Mem { start_time, .. } => *start_time,
        ContextAction::DirMem { start_time, .. } => *start_time,
        ContextAction::Get { start_time, .. } => *start_time,
        ContextAction::Fold { start_time, .. } => *start_time,
        ContextAction::Shutdown => 0_f64,
    }
}

impl Ord for ContextAction {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        get_time(&self)
            .partial_cmp(&get_time(&other))
            .unwrap_or(Equal)
    }
}

impl PartialOrd for ContextAction {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ContextAction {
    fn eq(&self, other: &Self) -> bool {
        get_time(&self) == get_time(&other)
    }
}

impl Eq for ContextAction {}
