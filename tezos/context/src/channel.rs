// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam::channel::{bounded, Receiver, RecvError, Sender, SendError};
use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;

static CHANNEL_ENABLED: AtomicBool = AtomicBool::new(false);
const CHANNEL_BUFFER_LEN: usize = 1_048_576;

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

type Hash = Vec<u8>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum ContextAction {
    Set {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        value: Vec<u8>,
        value_as_json: Option<String>,
        start_time: f64,
        end_time: f64,
    },
    Delete {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    RemoveRecord {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    Copy {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        from_key: Vec<String>,
        to_key: Vec<String>,
        start_time: f64,
        end_time: f64,
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
        start_time: f64,
        end_time: f64,
    },
    Mem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    DirMem {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    Get {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    Fold {
        context_hash: Option<Hash>,
        block_hash: Option<Hash>,
        operation_hash: Option<Hash>,
        key: Vec<String>,
        start_time: f64,
        end_time: f64,
    },
    /// This is a control event used to shutdown IPC channel
    Shutdown,
}