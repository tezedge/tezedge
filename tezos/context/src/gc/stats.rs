// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crossbeam_channel::RecvError;

use super::worker::Command;

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MarkNewIds { new_ids } => f
                .debug_struct("MarkNewIds")
                .field("new_ids", &new_ids.len())
                .finish(),
            Self::BlockApplied {
                new_ids,
                commit_hash_id,
                block_level,
            } => f
                .debug_struct("Commit   ")
                .field("new_ids", &new_ids.len())
                .field("commit", commit_hash_id)
                .field("block_level", block_level)
                .finish(),
            Self::NewChunks {
                objects_chunks,
                hashes_chunks,
            } => {
                let objects_len = objects_chunks.as_ref().map(|c| c.len()).unwrap_or(0);
                let hashes_len = hashes_chunks.as_ref().map(|c| c.len()).unwrap_or(0);
                f.debug_struct("NewChunks")
                    .field("objects", &objects_len)
                    .field("hashes", &hashes_len)
                    .finish()
            }
            Self::StoreRepository { .. } => f.debug_struct("StoreRepository").finish(),
            Self::Close => write!(f, "Close"),
        }
    }
}

#[derive(Debug)]
pub struct CollectorStatistics {
    pub unused_found: usize,
    pub nobjects: usize,
    pub max_depth: usize,
    pub object_total_bytes: usize,
    pub gc_duration: std::time::Duration,
    pub objects_chunks_alive: usize,
    pub objects_chunks_dead: usize,
    pub hashes_chunks_alive: usize,
    pub hashes_chunks_dead: usize,
    pub delay_since_last_gc: Option<std::time::Duration>,
}

#[derive(Debug)]
pub struct CommitStatistics {
    pub new_hash_id: usize,
}

pub struct OnMessageStatistics<'a> {
    pub command: &'a Result<Command, RecvError>,
    pub pending_command: usize,
    pub pending_hash_ids_length: usize,
    pub pending_hash_ids_capacity: usize,
    pub objects_nchunks: usize,
    pub hashes_nchunks: usize,
    pub counter_nchunks: usize,
}

impl<'a> std::fmt::Debug for OnMessageStatistics<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let command = match self.command {
            Ok(cmd) => format!("{:?}", &cmd),
            e => format!("{:?}", e),
        };

        let command = format!("{:<55}", command);
        f.debug_struct("OnMessageStatistics")
            .field("command", &command)
            .field("pending_command", &self.pending_command)
            .field("pending_hash_ids_length", &self.pending_hash_ids_length)
            .field("pending_hash_ids_capacity", &self.pending_hash_ids_capacity)
            .field("objects_nchunks", &self.objects_nchunks)
            .field("hashes_nchunks", &self.hashes_nchunks)
            .field("counter_nchunks", &self.counter_nchunks)
            .finish()
    }
}
