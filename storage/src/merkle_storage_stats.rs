use serde::Serialize;
use std::cmp;
use std::collections::HashMap;
use std::fmt;
use std::time::Instant;

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
pub type OperationLatencyStats = HashMap<String, OperationLatencies>;

// Latency statistics per path indexed by first chunk of path (under /data/)
pub type PerPathOperationStats = HashMap<String, OperationLatencyStats>;

#[derive(Serialize, Debug, Clone)]
pub struct MerkleStoragePerfReport {
    pub perf_stats: MerklePerfStats,
    pub kv_store_stats: usize,
}

impl MerkleStoragePerfReport {
    pub fn new(perf_stats: MerklePerfStats, kv_store_stats: usize) -> Self {
        let mut perf = perf_stats;
        for (_, stat) in perf.global.iter_mut() {
            if stat.op_exec_times > 0 {
                stat.avg_exec_time = stat.cumul_op_exec_time as f64 / stat.op_exec_times as f64;
            } else {
                stat.avg_exec_time = 0.0;
            }
        }
        // calculate average values for per-path stats
        for (_node, stat) in perf.perpath.iter_mut() {
            for (_op, stat) in stat.iter_mut() {
                if stat.op_exec_times > 0 {
                    stat.avg_exec_time = stat.cumul_op_exec_time as f64 / stat.op_exec_times as f64;
                } else {
                    stat.avg_exec_time = 0.0;
                }
            }
        }
        MerkleStoragePerfReport {
            perf_stats: perf,
            kv_store_stats,
        }
    }
}

#[derive(Serialize, Default, Debug, Clone)]
pub struct MerklePerfStats {
    pub global: OperationLatencyStats,
    pub perpath: PerPathOperationStats,
}

#[derive(Serialize, Default, Debug, Clone)]
pub struct MerkleStorageStatistics {
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
    GarbageCollector,
}

impl fmt::Display for MerkleStorageAction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MerkleStorageAction::Set => {
                write!(f, "Set").unwrap();
            }
            MerkleStorageAction::Get => {
                write!(f, "Get").unwrap();
            }
            MerkleStorageAction::GetByPrefix => {
                write!(f, "GetByPrefix").unwrap();
            }
            MerkleStorageAction::GetKeyValuesByPrefix => {
                write!(f, "GetKeyValuesByPrefix").unwrap();
            }
            MerkleStorageAction::GetContextTreeByPrefix => {
                write!(f, "GetContextTreeByPrefix").unwrap();
            }
            MerkleStorageAction::GetHistory => {
                write!(f, "GetHistory").unwrap();
            }
            MerkleStorageAction::Mem => {
                write!(f, "Mem").unwrap();
            }
            MerkleStorageAction::DirMem => {
                write!(f, "DirMem").unwrap();
            }
            MerkleStorageAction::Copy => {
                write!(f, "Copy").unwrap();
            }
            MerkleStorageAction::Delete => {
                write!(f, "Delete").unwrap();
            }
            MerkleStorageAction::DeleteRecursively => {
                write!(f, "DeleteRecursively").unwrap();
            }
            MerkleStorageAction::Commit => {
                write!(f, "Commit").unwrap();
            }
            MerkleStorageAction::Checkout => {
                write!(f, "Checkout").unwrap();
            }
            MerkleStorageAction::GarbageCollector => {
                write!(f, "GarbageCollector").unwrap();
            }
        };
        Ok(())
    }
}

pub struct StatUpdater<'a> {
    stats: &'a mut MerkleStorageStatistics,
    timer: Instant,
    action: MerkleStorageAction,
    key: Option<String>,
}

impl<'a> Drop for StatUpdater<'a> {
    fn drop(&mut self) {
        self.update_execution_stats()
    }
}

impl<'a> StatUpdater<'a> {
    pub fn new(
        stats: &'a mut MerkleStorageStatistics,
        action: MerkleStorageAction,
        action_key: Option<&Vec<String>>,
    ) -> Self {
        let key = match action_key {
            Some(path) => {
                if path.len() > 1 && path[0] == "data" {
                    Some(path[1].to_string())
                } else {
                    None
                }
            }
            None => None,
        };

        StatUpdater {
            stats,
            timer: Instant::now(),
            action,
            key,
        }
    }

    pub fn update_execution_stats(&mut self) {
        // stop timer and get duration
        let exec_time_nanos = self.timer.elapsed().as_nanos();

        self.stats.block_latencies.update(exec_time_nanos as u64);
        // commit signifies end of the block

        if let MerkleStorageAction::Commit = self.action {
            self.stats.block_latencies.end_block();
        }

        // per action stats
        let entry = self
            .stats
            .perf_stats
            .global
            .entry(self.action.to_string())
            .or_insert_with(OperationLatencies::default);
        entry.cumul_op_exec_time += exec_time_nanos;
        entry.op_exec_times += 1;
        entry.op_exec_time_min = cmp::min(exec_time_nanos, entry.op_exec_time_min);
        entry.op_exec_time_max = cmp::max(exec_time_nanos, entry.op_exec_time_max);

        // per-path stats
        if let Some(path) = &self.key {
            let perpath = self
                .stats
                .perf_stats
                .perpath
                .entry(path.clone())
                .or_insert_with(HashMap::new);

            let entry = perpath
                .entry(self.action.to_string())
                .or_insert_with(OperationLatencies::default);

            entry.cumul_op_exec_time += exec_time_nanos;
            entry.op_exec_times += 1;
            entry.op_exec_time_min = cmp::min(exec_time_nanos, entry.op_exec_time_min);
            entry.op_exec_time_max = cmp::max(exec_time_nanos, entry.op_exec_time_max);
        }
    }
}
