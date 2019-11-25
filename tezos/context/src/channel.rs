// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam::channel::{bounded, Receiver, RecvError, Sender, SendError};
use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;

use crate::{ContextHash, ContextKey, ContextValue, BlockHash, OperationHash};

static CHANNEL_ENABLED: AtomicBool = AtomicBool::new(false);
const CHANNEL_BUFFER_LEN: usize = 131_072;

lazy_static! {
    static ref CHANNEL: (Sender<ContextAction>, Receiver<ContextAction>) = bounded(CHANNEL_BUFFER_LEN);
}

pub fn context_send(action: ContextAction) -> Result<(), SendError<ContextAction>> {
    if CHANNEL_ENABLED.load(Ordering::Acquire) {
        CHANNEL.0.send(action)
    } else {
        Ok(())
    }
}

pub fn context_receive() -> Result<ContextAction, RecvError> {
    CHANNEL.1.recv()
}

/// By default channel is disabled.
pub fn enable_context_channel() {
    CHANNEL_ENABLED.store(true, Ordering::Release)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ContextAction {
    Set {
        context_hash: Option<ContextHash>,
        block_hash: Option<BlockHash>,
        operation_hash: Option<OperationHash>,
        key: ContextKey,
        value: ContextValue,
    },
    Delete {
        context_hash: Option<ContextHash>,
        block_hash: Option<BlockHash>,
        operation_hash: Option<OperationHash>,
        key: ContextKey,
    },
    RemoveRecord {
        context_hash: Option<ContextHash>,
        block_hash: Option<BlockHash>,
        operation_hash: Option<OperationHash>,
        key: ContextKey,
    },
    Copy {
        context_hash: Option<ContextHash>,
        block_hash: Option<BlockHash>,
        operation_hash: Option<OperationHash>,
        from_key: ContextKey,
        to_key: ContextKey,
    },
    Checkout {
        context_hash: ContextHash,
    },
    Commit {
        parent_context_hash: Option<ContextHash>,
        block_hash: Option<BlockHash>,
        new_context_hash: ContextHash,
    },
    /// This is a control event used to shutdown IPC channel
    Shutdown,
}