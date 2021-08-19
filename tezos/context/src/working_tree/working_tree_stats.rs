use serde::Serialize;
use std::cmp;
use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

// TODO: move up one directory, these are stats for the context

/// Latency statistics for each action (in nanoseconds)
#[derive(Serialize, Debug, Clone, Copy)]
pub struct OperationLatencies {
    /// divide this by the next field to get avg (mean) time spent in operation
    pub cumul_op_exec_time: u128,
    pub op_exec_times: u128,
    pub avg_exec_time: f64,
    /// lowest time spent in operation
    pub op_exec_time_min: u128,
    /// highest time spent in operation
    pub op_exec_time_max: u128,
}

impl Default for OperationLatencies {
    fn default() -> Self {
        OperationLatencies {
            cumul_op_exec_time: 0,
            op_exec_times: 0,
            avg_exec_time: 0.0,
            op_exec_time_min: u128::MAX,
            op_exec_time_max: u128::MIN,
        }
    }
}

/// Block application latencies
#[derive(Serialize, Default, Debug, Clone)]
pub struct BlockLatencies {
    latencies: Vec<u64>,
    current: u64,
}

// Latency statistics indexed by operation name (e.g. "Set")
pub type OperationLatencyStats = HashMap<MerkleStorageAction, OperationLatencies>;

// Latency statistics per path indexed by first chunk of path (under /data/)
pub type PerPathOperationStats = HashMap<String, OperationLatencyStats>;

#[derive(Serialize, Debug, Clone, Default)]
pub struct MerkleStoragePerfReport {
    pub perf_stats: MerklePerfStats,
    pub kv_store_stats: usize,
}

#[derive(Serialize, Default, Debug, Clone)]
pub struct MerklePerfStats {
    pub global: OperationLatencyStats,
    pub perpath: PerPathOperationStats,
}

#[derive(Serialize, Default, Debug, Clone)]
pub struct TezedgeContextStatistics {
    pub perf_stats: MerklePerfStats,
    pub block_latencies: BlockLatencies,
}

impl BlockLatencies {
    fn update(&mut self, latency: u64) {
        self.current += latency;
    }

    fn end_block(&mut self) {
        self.latencies.push(self.current);
        self.current = 0;
    }

    pub fn get(&self, offset_from_last_applied: usize) -> Option<u64> {
        self.latencies
            .len()
            .checked_sub(offset_from_last_applied + 1)
            .and_then(|index| self.latencies.get(index))
            .copied()
    }
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MerkleStorageAction {
    Set,
    Get,
    GetByPrefix,
    GetKeyValuesByPrefix,
    GetContextTreeByPrefix,
    GetHistory,
    Mem,
    DirMem,
    Copy,
    Delete,
    DeleteRecursively,
    Commit,
    Checkout,
    BlockApplied,
}

impl fmt::Display for MerkleStorageAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MerkleStorageAction::Set => {
                write!(f, "Set")
            }
            MerkleStorageAction::Get => {
                write!(f, "Get")
            }
            MerkleStorageAction::GetByPrefix => {
                write!(f, "GetByPrefix")
            }
            MerkleStorageAction::GetKeyValuesByPrefix => {
                write!(f, "GetKeyValuesByPrefix")
            }
            MerkleStorageAction::GetContextTreeByPrefix => {
                write!(f, "GetContextTreeByPrefix")
            }
            MerkleStorageAction::GetHistory => {
                write!(f, "GetHistory")
            }
            MerkleStorageAction::Mem => {
                write!(f, "Mem")
            }
            MerkleStorageAction::DirMem => {
                write!(f, "DirMem")
            }
            MerkleStorageAction::Copy => {
                write!(f, "Copy")
            }
            MerkleStorageAction::Delete => {
                write!(f, "Delete")
            }
            MerkleStorageAction::DeleteRecursively => {
                write!(f, "DeleteRecursively")
            }
            MerkleStorageAction::Commit => {
                write!(f, "Commit")
            }
            MerkleStorageAction::Checkout => {
                write!(f, "Checkout")
            }
            MerkleStorageAction::BlockApplied => {
                write!(f, "GC")
            }
        }
    }
}

pub struct StatUpdater {
    timer: Instant,
    action: MerkleStorageAction,
    key: Option<String>,
}

impl StatUpdater {
    pub fn new(action: MerkleStorageAction, action_key: Option<&Vec<String>>) -> Self {
        let key = match action_key {
            Some(path) if path.len() > 1 && path[0] == "data" => Some(path[1].to_string()),
            Some(_) | None => None,
        };

        StatUpdater {
            timer: Instant::now(),
            action,
            key,
        }
    }

    pub fn update_execution_stats(&self, stats: &mut TezedgeContextStatistics) {
        // stop timer and get duration
        let exec_time_nanos = self.timer.elapsed().as_nanos();

        stats.block_latencies.update(exec_time_nanos as u64);
        // commit signifies end of the block

        if let MerkleStorageAction::BlockApplied = self.action {
            stats.block_latencies.end_block();
        }

        // per action stats
        let entry = stats
            .perf_stats
            .global
            .entry(self.action.clone())
            .or_insert_with(OperationLatencies::default);
        entry.cumul_op_exec_time += exec_time_nanos;
        entry.op_exec_times += 1;
        entry.avg_exec_time = entry.cumul_op_exec_time as f64 / entry.op_exec_times as f64;
        entry.op_exec_time_min = cmp::min(exec_time_nanos, entry.op_exec_time_min);
        entry.op_exec_time_max = cmp::max(exec_time_nanos, entry.op_exec_time_max);

        // per-path stats
        if let Some(path) = &self.key {
            let perpath = stats
                .perf_stats
                .perpath
                .entry(path.clone())
                .or_insert_with(HashMap::new);

            let entry = perpath
                .entry(self.action.clone())
                .or_insert_with(OperationLatencies::default);

            entry.cumul_op_exec_time += exec_time_nanos;
            entry.op_exec_times += 1;
            entry.avg_exec_time = entry.cumul_op_exec_time as f64 / entry.op_exec_times as f64;
            entry.op_exec_time_min = cmp::min(exec_time_nanos, entry.op_exec_time_min);
            entry.op_exec_time_max = cmp::max(exec_time_nanos, entry.op_exec_time_max);
        }
    }
}
