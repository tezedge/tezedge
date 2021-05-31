// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    convert::TryInto,
    path::PathBuf,
    time::{Duration, Instant},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use once_cell::sync::Lazy;
use rusqlite::{named_params, Batch, Connection, Error as SQLError};
use serde::Serialize;

pub const FILENAME_DB: &str = "context_stats.db";

#[derive(Debug)]
pub enum ActionKind {
    Mem,
    MemTree,
    Find,
    FindTree,
    Add,
    AddTree,
    Remove,
}

impl ActionKind {
    fn to_str(&self) -> &'static str {
        match self {
            ActionKind::Mem => "mem",
            ActionKind::MemTree => "mem_tree",
            ActionKind::Find => "find",
            ActionKind::FindTree => "find_tree",
            ActionKind::Add => "add",
            ActionKind::AddTree => "add_tree",
            ActionKind::Remove => "remove",
        }
    }
}

// TODO: add tree_action

#[derive(Debug)]
pub enum TimingMessage {
    SetBlock {
        block_hash: Option<BlockHash>,
        start_at: Option<(Duration, Instant)>,
    },
    SetOperation(Option<OperationHash>),
    Checkout {
        context_hash: ContextHash,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    },
    Commit {
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    },
    Action(Action),
    InitTiming {
        db_path: Option<PathBuf>,
    },
}

// Id of the hash in the database
type HashId = String;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DetailedTime {
    pub count: usize,
    pub mean_time: f64,
    pub max_time: f64,
    pub total_time: f64,
}

impl DetailedTime {
    fn compute_mean(&mut self) {
        let mean = self.total_time / self.count as f64;
        self.mean_time = mean.max(0.0);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RangeStats {
    pub total_time: f64,
    pub actions_count: usize,
    pub one_to_ten_us: DetailedTime,
    pub ten_to_one_hundred_us: DetailedTime,
    pub one_hundred_us_to_one_ms: DetailedTime,
    pub one_to_ten_ms: DetailedTime,
    pub ten_to_one_hundred_ms: DetailedTime,
    pub one_hundred_ms_to_one_s: DetailedTime,
    pub one_to_ten_s: DetailedTime,
    pub ten_to_one_hundred_s: DetailedTime,
    pub one_hundred_s: DetailedTime,
}

impl RangeStats {
    fn compute_mean(&mut self) {
        self.one_to_ten_us.compute_mean();
        self.ten_to_one_hundred_us.compute_mean();
        self.one_hundred_us_to_one_ms.compute_mean();
        self.one_to_ten_ms.compute_mean();
        self.ten_to_one_hundred_ms.compute_mean();
        self.one_hundred_ms_to_one_s.compute_mean();
        self.one_to_ten_s.compute_mean();
        self.ten_to_one_hundred_s.compute_mean();
        self.one_hundred_s.compute_mean();
    }

    fn add_time<T: Into<Option<f64>>>(&mut self, time: T) {
        let time = match time.into() {
            Some(t) => t,
            None => return,
        };

        self.total_time += time;
        self.actions_count = self.actions_count.saturating_add(1);

        let entry = match time {
            t if t < 0.00001 => &mut self.one_to_ten_us,
            t if t < 0.0001 => &mut self.ten_to_one_hundred_us,
            t if t < 0.001 => &mut self.one_hundred_us_to_one_ms,
            t if t < 0.01 => &mut self.one_to_ten_ms,
            t if t < 0.1 => &mut self.ten_to_one_hundred_ms,
            t if t < 1.0 => &mut self.one_hundred_ms_to_one_s,
            t if t < 10.0 => &mut self.one_to_ten_s,
            t if t < 100.0 => &mut self.ten_to_one_hundred_s,
            _ => &mut self.one_hundred_s,
        };
        entry.count = entry.count.saturating_add(1);
        entry.total_time += time;
        entry.max_time = entry.max_time.max(time);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionStatsWithRange {
    pub root: String,
    pub total_time_read: f64,
    pub total_time_write: f64,
    pub total_time: f64,
    pub actions_count: usize,
    pub mem: RangeStats,
    pub mem_tree: RangeStats,
    pub find: RangeStats,
    pub find_tree: RangeStats,
    pub add: RangeStats,
    pub add_tree: RangeStats,
    pub remove: RangeStats,
}

impl ActionStatsWithRange {
    fn compute_mean(&mut self) {
        self.mem.compute_mean();
        self.find.compute_mean();
        self.find_tree.compute_mean();
        self.add.compute_mean();
        self.add_tree.compute_mean();
        self.remove.compute_mean();
    }

    pub fn compute_total(&mut self) {
        self.total_time_read =
            self.mem.total_time + self.find.total_time + self.find_tree.total_time;

        self.total_time_write =
            self.add.total_time + self.add_tree.total_time + self.remove.total_time;

        self.total_time = self.total_time_read + self.total_time_write;

        self.actions_count = self
            .mem
            .actions_count
            .saturating_add(self.find.actions_count)
            .saturating_add(self.find_tree.actions_count)
            .saturating_add(self.add.actions_count)
            .saturating_add(self.add_tree.actions_count)
            .saturating_add(self.remove.actions_count);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionData {
    pub root: String,
    pub tezedge_count: usize,
    pub irmin_count: usize,
    pub tezedge_mean_time: f64,
    pub tezedge_max_time: f64,
    pub tezedge_total_time: f64,
    pub irmin_mean_time: f64,
    pub irmin_max_time: f64,
    pub irmin_total_time: f64,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionStats {
    pub data: ActionData,
    pub tezedge_mem: f64,
    pub tezedge_mem_tree: f64,
    pub tezedge_find: f64,
    pub tezedge_find_tree: f64,
    pub tezedge_add: f64,
    pub tezedge_add_tree: f64,
    pub tezedge_remove: f64,
    pub irmin_mem: f64,
    pub irmin_mem_tree: f64,
    pub irmin_find: f64,
    pub irmin_find_tree: f64,
    pub irmin_add: f64,
    pub irmin_add_tree: f64,
    pub irmin_remove: f64,
}

impl ActionStats {
    fn compute_mean(&mut self) {
        let mean = self.data.tezedge_total_time / self.data.tezedge_count as f64;
        self.data.tezedge_mean_time = mean.max(0.0);

        let mean = self.data.irmin_total_time / self.data.irmin_count as f64;
        self.data.irmin_mean_time = mean.max(0.0);
    }
}

struct Timing {
    current_block: Option<(HashId, BlockHash)>,
    current_operation: Option<(HashId, OperationHash)>,
    current_context: Option<(HashId, ContextHash)>,
    block_started_at: Option<(Duration, Instant)>,
    /// Number of actions in current block
    nactions: usize,
    /// Checkout time for the current block
    checkout_time: Option<(Option<f64>, Option<f64>)>,
    /// Statistics for the current block
    block_stats: HashMap<String, ActionStats>,
    /// Global statistics
    tezedge_global_stats: HashMap<String, ActionStatsWithRange>,
    irmin_global_stats: HashMap<String, ActionStatsWithRange>,
    tezedge_commit_stats: RangeStats,
    irmin_commit_stats: RangeStats,
    tezedge_checkout_stats: RangeStats,
    irmin_checkout_stats: RangeStats,
    sql: Connection,
}

impl std::fmt::Debug for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timing")
            .field("current_block", &self.current_block)
            .field("current_operation", &self.current_operation)
            .field("current_context", &self.current_context)
            .finish()
    }
}

#[derive(Debug)]
pub struct Action {
    pub action_name: ActionKind,
    pub key: Vec<String>,
    pub irmin_time: Option<f64>,
    pub tezedge_time: Option<f64>,
}

pub static TIMING_CHANNEL: Lazy<Sender<TimingMessage>> = Lazy::new(|| {
    let (sender, receiver) = unbounded();

    std::thread::Builder::new()
        .name("timing".to_string())
        .spawn(|| {
            start_timing(receiver);
        })
        .unwrap();

    sender
});

fn start_timing(recv: Receiver<TimingMessage>) {
    let mut db_path: Option<PathBuf> = None;

    for msg in &recv {
        match msg {
            TimingMessage::InitTiming { db_path: path } => {
                db_path = path.clone();
                break;
            }
            _ => {}
        }
    }

    let mut timing = Timing::new(db_path);

    for msg in recv {
        if let Err(_err) = timing.process_msg(msg) {
            // TODO: log error, and retry
        }
    }
}

pub fn hash_to_string(hash: &[u8]) -> String {
    const HEXCHARS: &[u8] = b"0123456789abcdef";

    let mut s = String::with_capacity(62);
    for byte in hash {
        s.push(HEXCHARS[*byte as usize >> 4] as char);
        s.push(HEXCHARS[*byte as usize & 0xF] as char);
    }
    s
}

impl Timing {
    fn new(db_path: Option<PathBuf>) -> Timing {
        let sql = Self::init_sqlite(db_path).unwrap();

        Timing {
            current_block: None,
            current_operation: None,
            current_context: None,
            block_started_at: None,
            nactions: 0,
            checkout_time: None,
            block_stats: HashMap::default(),
            tezedge_global_stats: HashMap::default(),
            irmin_global_stats: HashMap::default(),
            tezedge_commit_stats: Default::default(),
            irmin_commit_stats: RangeStats::default(),
            tezedge_checkout_stats: Default::default(),
            irmin_checkout_stats: RangeStats::default(),
            sql,
        }
    }

    fn process_msg(&mut self, msg: TimingMessage) -> Result<(), SQLError> {
        match msg {
            TimingMessage::SetBlock {
                block_hash,
                start_at,
            } => self.set_current_block(block_hash, start_at),
            TimingMessage::SetOperation(operation_hash) => {
                self.set_current_operation(operation_hash)
            }
            TimingMessage::Action(action) => self.insert_action(&action),
            TimingMessage::Checkout {
                context_hash,
                irmin_time,
                tezedge_time,
            } => self.insert_checkout(context_hash, irmin_time, tezedge_time),
            TimingMessage::Commit {
                irmin_time,
                tezedge_time,
            } => self.insert_commit(irmin_time, tezedge_time),
            TimingMessage::InitTiming { .. } => Ok(()),
        }
    }

    fn set_current_block(
        &mut self,
        block_hash: Option<BlockHash>,
        mut start_at: Option<(Duration, Instant)>,
    ) -> Result<(), SQLError> {
        if let Some(start_at) = start_at.take() {
            self.block_started_at = Some(start_at);
        } else if let Some((timestamp, instant)) = self.block_started_at.take() {
            let duration_millis: u64 = instant.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
            let timestamp_secs = timestamp.as_secs();
            let timestamp_nanos = timestamp.subsec_nanos();
            let block_id = self.current_block.as_ref().unwrap().0.as_str();

            let mut query = self.sql.prepare_cached(
                "
            UPDATE
              blocks
            SET
              timestamp_secs = :timestamp_secs,
              timestamp_nanos = :timestamp_nanos,
              duration_millis = :duration
            WHERE
              id = :block_id;
                ",
            )?;

            query.execute(named_params! {
                ":timestamp_secs": timestamp_secs,
                ":timestamp_nanos": timestamp_nanos,
                ":duration": duration_millis,
                ":block_id": block_id,
            })?;
        }

        Self::set_current(&self.sql, block_hash, &mut self.current_block, "blocks")?;

        // Reset context and operation
        self.current_context = None;
        self.current_operation = None;
        self.checkout_time = None;
        self.nactions = 0;
        self.block_stats = HashMap::default();

        Ok(())
    }

    fn set_current_operation(
        &mut self,
        operation_hash: Option<OperationHash>,
    ) -> Result<(), SQLError> {
        Self::set_current(
            &self.sql,
            operation_hash,
            &mut self.current_operation,
            "operations",
        )
    }

    fn set_current_context(&mut self, context_hash: ContextHash) -> Result<(), SQLError> {
        Self::set_current(
            &self.sql,
            Some(context_hash),
            &mut self.current_context,
            "contexts",
        )
    }

    fn set_current<T>(
        sql: &Connection,
        hash: Option<T>,
        current: &mut Option<(HashId, T)>,
        table_name: &str,
    ) -> Result<(), SQLError>
    where
        T: Eq,
        T: AsRef<Vec<u8>>,
    {
        match (hash.as_ref(), current.as_ref()) {
            (None, _) => {
                *current = None;
                return Ok(());
            }
            (Some(hash), Some((_, current_hash))) if hash == current_hash => {
                return Ok(());
            }
            _ => {}
        };

        let hash = hash.unwrap();
        let hash_string = hash_to_string(hash.as_ref());

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string)? {
            current.replace((id.to_string(), hash));
            return Ok(());
        };

        // FIXME: this "OR IGNORE" is only valid for checkouts, if we get a duplicate when doing something
        // else there may be a problem.
        // Related to this, in general we want to avoid except/unwrap, because this will kill the thread and
        // everything will stop working, it should be handled more gracefully if possible.
        // Not a priority right now but we have to think about how to properly handle such situations, specially
        // if this functionality gets extended to include more data about the context actions.
        sql.execute(
            &format!(
                "INSERT OR IGNORE INTO {table} (hash) VALUES (?1);",
                table = table_name
            ),
            [&hash_string],
        )?;

        let id = Self::get_id_on_table(sql, table_name, &hash_string)?
            .expect("Unable to find row after INSERT"); // This should never happen

        current.replace((id.to_string(), hash));

        Ok(())
    }

    fn get_id_on_table(
        sql: &Connection,
        table_name: &str,
        hash_string: &str,
    ) -> Result<Option<i64>, SQLError> {
        let mut stmt = sql.prepare(&format!(
            "SELECT id FROM {table} WHERE hash = ?1;",
            table = table_name
        ))?;
        let mut rows = stmt.query([hash_string])?;

        if let Some(row) = rows.next()? {
            return Ok(Some(row.get(0)?));
        };

        Ok(None)
    }

    fn insert_checkout(
        &mut self,
        context_hash: ContextHash,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.tezedge_checkout_stats.add_time(tezedge_time);
        self.irmin_checkout_stats.add_time(irmin_time);
        self.set_current_context(context_hash)?;
        self.checkout_time = Some((irmin_time, tezedge_time));

        Ok(())
    }

    fn insert_commit(
        &mut self,
        irmin_time: Option<f64>,
        tezedge_time: Option<f64>,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.tezedge_commit_stats.add_time(tezedge_time);
        self.irmin_commit_stats.add_time(irmin_time);
        self.sync_global_stats(irmin_time, tezedge_time)?;
        self.sync_block_stats()?;

        Ok(())
    }

    fn insert_action(&mut self, action: &Action) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());
        let operation_id = self.current_operation.as_ref().map(|(id, _)| id.as_str());
        let context_id = self.current_context.as_ref().map(|(id, _)| id.as_str());
        let action_name = action.action_name.to_str();

        let (root, key_id) = if action.key.is_empty() {
            (None, None)
        } else {
            let root = action.key[0].as_str();
            let key = action.key.join("/");

            let mut stmt = self
                .sql
                .prepare_cached("INSERT OR IGNORE INTO keys (key) VALUES (?1)")?;

            stmt.execute([key.as_str()])?;

            let mut stmt = self
                .sql
                .prepare_cached("SELECT id FROM keys WHERE key = ?1;")?;

            let key_id: usize = stmt.query_row([key.as_str()], |row| row.get(0))?;

            (Some(root), Some(key_id))
        };

        let mut stmt = self.sql.prepare_cached(
            "
        INSERT INTO actions
          (name, key_root, key_id, irmin_time, tezedge_time, block_id, operation_id, context_id)
        VALUES
          (:name, :key_root, :key_id, :irmin_time, :tezedge_time, :block_id, :operation_id, :context_id);
            "
        )?;

        stmt.execute(named_params! {
            ":name": action_name,
            ":key_root": &root,
            ":key_id": &key_id,
            ":irmin_time": &action.irmin_time,
            ":tezedge_time": &action.tezedge_time,
            ":block_id": block_id,
            ":operation_id": operation_id,
            ":context_id": context_id
        })?;

        drop(stmt);

        self.nactions = self.nactions.saturating_add(1);

        let root = match root {
            Some(root) => root,
            None => return Ok(()),
        };

        self.add_block_stats(root, &action);
        self.add_global_stats(root, &action);

        Ok(())
    }

    fn add_global_stats(&mut self, root: &str, action: &Action) {
        for (global_stats, time) in &mut [
            (&mut self.tezedge_global_stats, action.tezedge_time),
            (&mut self.irmin_global_stats, action.irmin_time),
        ] {
            let time = match time {
                Some(time) => time,
                None => continue,
            };

            let entry = match global_stats.get_mut(root) {
                Some(entry) => entry,
                None => {
                    let stats = ActionStatsWithRange {
                        root: root.to_string(),
                        ..Default::default()
                    };
                    global_stats.insert(root.to_string(), stats);
                    global_stats.get_mut(root).unwrap()
                }
            };

            let time = *time;
            let action_stats = match action.action_name {
                ActionKind::Mem => &mut entry.mem,
                ActionKind::MemTree => &mut entry.mem_tree,
                ActionKind::Find => &mut entry.find,
                ActionKind::FindTree => &mut entry.find_tree,
                ActionKind::Add => &mut entry.add,
                ActionKind::AddTree => &mut entry.add_tree,
                ActionKind::Remove => &mut entry.remove,
            };

            entry.actions_count = entry.actions_count.saturating_add(1);
            entry.total_time += time;
            action_stats.add_time(time);
        }
    }

    fn add_block_stats(&mut self, root: &str, action: &Action) {
        let entry = match self.block_stats.get_mut(root) {
            Some(entry) => entry,
            None => {
                let mut stats = ActionStats::default();
                stats.data.root = root.to_string();
                self.block_stats.insert(root.to_string(), stats);
                self.block_stats.get_mut(root).unwrap()
            }
        };

        let (value_tezedge, value_irmin) = match action.action_name {
            ActionKind::Mem => (&mut entry.tezedge_mem, &mut entry.irmin_mem),
            ActionKind::MemTree => (&mut entry.tezedge_mem_tree, &mut entry.irmin_mem_tree),
            ActionKind::Find => (&mut entry.tezedge_find, &mut entry.irmin_find),
            ActionKind::FindTree => (&mut entry.tezedge_find_tree, &mut entry.irmin_find_tree),
            ActionKind::Add => (&mut entry.tezedge_add, &mut entry.irmin_add),
            ActionKind::AddTree => (&mut entry.tezedge_add_tree, &mut entry.irmin_add_tree),
            ActionKind::Remove => (&mut entry.tezedge_remove, &mut entry.irmin_remove),
        };

        if let Some(time) = action.tezedge_time {
            *value_tezedge += time;
            entry.data.tezedge_count = entry.data.tezedge_count.saturating_add(1);
            entry.data.tezedge_total_time += time;
            entry.data.tezedge_max_time = entry.data.tezedge_max_time.max(time);
        };

        if let Some(time) = action.irmin_time {
            *value_irmin += time;
            entry.data.irmin_count = entry.data.irmin_count.saturating_add(1);
            entry.data.irmin_total_time += time;
            entry.data.irmin_max_time = entry.data.irmin_max_time.max(time);
        };
    }

    fn sync_block_stats(&mut self) -> Result<(), SQLError> {
        for action in self.block_stats.values_mut() {
            action.compute_mean();
        }

        let block_id = self
            .current_block
            .as_ref()
            .map(|(id, _)| id.as_str())
            .unwrap();

        for (root, action_stats) in self.block_stats.iter() {
            let root = root.as_str();

            let mut query = self.sql.prepare_cached(
                "
            INSERT INTO block_action_stats
              (root, block_id, tezedge_count, irmin_count,
               tezedge_mean_time, tezedge_max_time, tezedge_total_time, tezedge_mem_time, tezedge_mem_tree_time, tezedge_find_time,
               tezedge_find_tree_time, tezedge_add_time, tezedge_add_tree_time, tezedge_remove_time,
               irmin_mean_time, irmin_max_time, irmin_total_time, irmin_mem_time, irmin_mem_tree_time, irmin_find_time,
               irmin_find_tree_time, irmin_add_time, irmin_add_tree_time, irmin_remove_time)
            VALUES
              (:root, :block_id, :tezedge_count, :irmin_count,
               :tezedge_mean_time, :tezedge_max_time, :tezedge_total_time, :tezedge_mem_time, :tezedge_mem_tree_time, :tezedge_find_time,
               :tezedge_find_tree_time, :tezedge_add_time, :tezedge_add_tree_time, :tezedge_remove_time,
               :irmin_mean_time, :irmin_max_time, :irmin_total_time, :irmin_mem_time, :irmin_mem_tree_time, :irmin_find_time,
               :irmin_find_tree_time, :irmin_add_time, :irmin_add_tree_time, :irmin_remove_time)
                ",
            )?;

            query.execute(named_params! {
                ":root": root,
                ":block_id": block_id,
                ":tezedge_count": action_stats.data.tezedge_count,
                ":irmin_count": action_stats.data.irmin_count,
                ":tezedge_mean_time": action_stats.data.tezedge_mean_time,
                ":tezedge_max_time": action_stats.data.tezedge_max_time,
                ":tezedge_total_time": action_stats.data.tezedge_total_time,
                ":irmin_mean_time": action_stats.data.irmin_mean_time,
                ":irmin_max_time": action_stats.data.irmin_max_time,
                ":irmin_total_time": action_stats.data.irmin_total_time,
                ":tezedge_mem_time": action_stats.tezedge_mem,
                ":tezedge_mem_tree_time": action_stats.tezedge_mem_tree,
                ":tezedge_add_time": action_stats.tezedge_add,
                ":tezedge_add_tree_time": action_stats.tezedge_add_tree,
                ":tezedge_find_time": action_stats.tezedge_find,
                ":tezedge_find_tree_time": action_stats.tezedge_find_tree,
                ":tezedge_remove_time": action_stats.tezedge_remove,
                ":irmin_mem_time": action_stats.irmin_mem,
                ":irmin_mem_tree_time": action_stats.irmin_mem_tree,
                ":irmin_add_time": action_stats.irmin_add,
                ":irmin_add_tree_time": action_stats.irmin_add_tree,
                ":irmin_find_time": action_stats.irmin_find,
                ":irmin_find_tree_time": action_stats.irmin_find_tree,
                ":irmin_remove_time": action_stats.irmin_remove,
            })?;
        }

        Ok(())
    }

    // Compute stats for the current block and global ones
    fn sync_global_stats(
        &mut self,
        commit_time_irmin: Option<f64>,
        commit_time_tezedge: Option<f64>,
    ) -> Result<(), SQLError> {
        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());

        let mut query = self.sql.prepare_cached(
            "
        UPDATE
          blocks
        SET
          actions_count = :actions_count,
          checkout_time_irmin = :checkout_time_irmin,
          checkout_time_tezedge = :checkout_time_tezedge,
          commit_time_irmin = :commit_time_irmin,
          commit_time_tezedge = :commit_time_tezedge
        WHERE
          id = :block_id;
            ",
        )?;

        query.execute(named_params! {
            ":actions_count": &self.nactions,
            ":checkout_time_irmin": &self.checkout_time.as_ref().map(|(irmin, _)| irmin),
            ":checkout_time_tezedge": &self.checkout_time.as_ref().map(|(_, tezedge)| tezedge),
            ":commit_time_irmin": &commit_time_irmin,
            ":commit_time_tezedge": &commit_time_tezedge,
            ":block_id": block_id
        })?;

        for action in self.tezedge_global_stats.values_mut() {
            action.compute_mean();
        }
        for action in self.irmin_global_stats.values_mut() {
            action.compute_mean();
        }
        self.tezedge_checkout_stats.compute_mean();
        self.irmin_checkout_stats.compute_mean();
        self.tezedge_commit_stats.compute_mean();
        self.irmin_commit_stats.compute_mean();

        for (global_stats, commits, checkouts, name) in &mut [
            (
                &self.tezedge_global_stats,
                &self.tezedge_commit_stats,
                &self.tezedge_checkout_stats,
                "tezedge",
            ),
            (
                &self.irmin_global_stats,
                &self.irmin_commit_stats,
                &self.irmin_checkout_stats,
                "irmin",
            ),
        ] {
            for (root, action_stats) in global_stats.iter() {
                let root = root.as_str();

                self.insert_action_stats(name, root, "mem", &action_stats.mem)?;
                self.insert_action_stats(name, root, "mem_tree", &action_stats.mem_tree)?;
                self.insert_action_stats(name, root, "find", &action_stats.find)?;
                self.insert_action_stats(name, root, "find_tree", &action_stats.find_tree)?;
                self.insert_action_stats(name, root, "add", &action_stats.add)?;
                self.insert_action_stats(name, root, "add_tree", &action_stats.add_tree)?;
                self.insert_action_stats(name, root, "remove", &action_stats.remove)?;
            }

            self.insert_action_stats(name, "commit", "commit", commits)?;
            self.insert_action_stats(name, "checkout", "checkout", checkouts)?;
        }

        Ok(())
    }

    fn insert_action_stats(
        &self,
        context_name: &str,
        root: &str,
        action_name: &str,
        range_stats: &RangeStats,
    ) -> Result<(), SQLError> {
        let mut query = self.sql.prepare_cached(
            "
        INSERT OR IGNORE INTO global_action_stats
          (root, action_name, context_name)
        VALUES
          (:root, :action_name, :context_name)
            ",
        )?;

        query.execute(named_params! {
            ":root": root,
            ":action_name": action_name,
            ":context_name": context_name,
        })?;

        let mut query = self.sql.prepare_cached(
            "
        UPDATE
          global_action_stats
        SET
          total_time = :total_time,
          actions_count = :actions_count,
          one_to_ten_us_count = :one_to_ten_us_count,
          one_to_ten_us_mean_time = :one_to_ten_us_mean_time,
          one_to_ten_us_max_time = :one_to_ten_us_max_time,
          one_to_ten_us_total_time = :one_to_ten_us_total_time,
          ten_to_one_hundred_us_count = :ten_to_one_hundred_us_count,
          ten_to_one_hundred_us_mean_time = :ten_to_one_hundred_us_mean_time,
          ten_to_one_hundred_us_max_time = :ten_to_one_hundred_us_max_time,
          ten_to_one_hundred_us_total_time = :ten_to_one_hundred_us_total_time,
          one_hundred_us_to_one_ms_count = :one_hundred_us_to_one_ms_count,
          one_hundred_us_to_one_ms_mean_time = :one_hundred_us_to_one_ms_mean_time,
          one_hundred_us_to_one_ms_max_time = :one_hundred_us_to_one_ms_max_time,
          one_hundred_us_to_one_ms_total_time = :one_hundred_us_to_one_ms_total_time,
          one_to_ten_ms_count = :one_to_ten_ms_count,
          one_to_ten_ms_mean_time = :one_to_ten_ms_mean_time,
          one_to_ten_ms_max_time = :one_to_ten_ms_max_time,
          one_to_ten_ms_total_time = :one_to_ten_ms_total_time,
          ten_to_one_hundred_ms_count = :ten_to_one_hundred_ms_count,
          ten_to_one_hundred_ms_mean_time = :ten_to_one_hundred_ms_mean_time,
          ten_to_one_hundred_ms_max_time = :ten_to_one_hundred_ms_max_time,
          ten_to_one_hundred_ms_total_time = :ten_to_one_hundred_ms_total_time,
          one_hundred_ms_to_one_s_count = :one_hundred_ms_to_one_s_count,
          one_hundred_ms_to_one_s_mean_time = :one_hundred_ms_to_one_s_mean_time,
          one_hundred_ms_to_one_s_max_time = :one_hundred_ms_to_one_s_max_time,
          one_hundred_ms_to_one_s_total_time = :one_hundred_ms_to_one_s_total_time,
          one_to_ten_s_count = :one_to_ten_s_count,
          one_to_ten_s_mean_time = :one_to_ten_s_mean_time,
          one_to_ten_s_max_time = :one_to_ten_s_max_time,
          one_to_ten_s_total_time = :one_to_ten_s_total_time,
          ten_to_one_hundred_s_count = :ten_to_one_hundred_s_count,
          ten_to_one_hundred_s_mean_time = :ten_to_one_hundred_s_mean_time,
          ten_to_one_hundred_s_max_time = :ten_to_one_hundred_s_max_time,
          ten_to_one_hundred_s_total_time = :ten_to_one_hundred_s_total_time,
          one_hundred_s_count = :one_hundred_s_count,
          one_hundred_s_mean_time = :one_hundred_s_mean_time,
          one_hundred_s_max_time = :one_hundred_s_max_time,
          one_hundred_s_total_time = :one_hundred_s_total_time
        WHERE
          root = :root AND action_name = :action_name AND context_name = :context_name;
        ",
        )?;

        query.execute(
            named_params! {
                ":root": root,
                ":action_name": action_name,
                ":context_name": context_name,
                ":total_time": &range_stats.total_time,
                ":actions_count": &range_stats.actions_count,
                ":one_to_ten_us_count": &range_stats.one_to_ten_us.count,
                ":one_to_ten_us_mean_time": &range_stats.one_to_ten_us.mean_time,
                ":one_to_ten_us_max_time": &range_stats.one_to_ten_us.max_time,
                ":one_to_ten_us_total_time": &range_stats.one_to_ten_us.total_time,
                ":ten_to_one_hundred_us_count": &range_stats.ten_to_one_hundred_us.count,
                ":ten_to_one_hundred_us_mean_time": &range_stats.ten_to_one_hundred_us.mean_time,
                ":ten_to_one_hundred_us_max_time": &range_stats.ten_to_one_hundred_us.max_time,
                ":ten_to_one_hundred_us_total_time": &range_stats.ten_to_one_hundred_us.total_time,
                ":one_hundred_us_to_one_ms_count": &range_stats.one_hundred_us_to_one_ms.count,
                ":one_hundred_us_to_one_ms_mean_time": &range_stats.one_hundred_us_to_one_ms.mean_time,
                ":one_hundred_us_to_one_ms_max_time": &range_stats.one_hundred_us_to_one_ms.max_time,
                ":one_hundred_us_to_one_ms_total_time": &range_stats.one_hundred_us_to_one_ms.total_time,
                ":one_to_ten_ms_count": &range_stats.one_to_ten_ms.count,
                ":one_to_ten_ms_mean_time": &range_stats.one_to_ten_ms.mean_time,
                ":one_to_ten_ms_max_time": &range_stats.one_to_ten_ms.max_time,
                ":one_to_ten_ms_total_time": &range_stats.one_to_ten_ms.total_time,
                ":ten_to_one_hundred_ms_count": &range_stats.ten_to_one_hundred_ms.count,
                ":ten_to_one_hundred_ms_mean_time": &range_stats.ten_to_one_hundred_ms.mean_time,
                ":ten_to_one_hundred_ms_max_time": &range_stats.ten_to_one_hundred_ms.max_time,
                ":ten_to_one_hundred_ms_total_time": &range_stats.ten_to_one_hundred_ms.total_time,
                ":one_hundred_ms_to_one_s_count": &range_stats.one_hundred_ms_to_one_s.count,
                ":one_hundred_ms_to_one_s_mean_time": &range_stats.one_hundred_ms_to_one_s.mean_time,
                ":one_hundred_ms_to_one_s_max_time": &range_stats.one_hundred_ms_to_one_s.max_time,
                ":one_hundred_ms_to_one_s_total_time": &range_stats.one_hundred_ms_to_one_s.total_time,
                ":one_to_ten_s_count": &range_stats.one_to_ten_s.count,
                ":one_to_ten_s_mean_time": &range_stats.one_to_ten_s.mean_time,
                ":one_to_ten_s_max_time": &range_stats.one_to_ten_s.max_time,
                ":one_to_ten_s_total_time": &range_stats.one_to_ten_s.total_time,
                ":ten_to_one_hundred_s_count": &range_stats.ten_to_one_hundred_s.count,
                ":ten_to_one_hundred_s_mean_time": &range_stats.ten_to_one_hundred_s.mean_time,
                ":ten_to_one_hundred_s_max_time": &range_stats.ten_to_one_hundred_s.max_time,
                ":ten_to_one_hundred_s_total_time": &range_stats.ten_to_one_hundred_s.total_time,
                ":one_hundred_s_count": &range_stats.one_hundred_s.count,
                ":one_hundred_s_mean_time": &range_stats.one_hundred_s.mean_time,
                ":one_hundred_s_max_time": &range_stats.one_hundred_s.max_time,
                ":one_hundred_s_total_time": &range_stats.one_hundred_s.total_time,
            },
        )?;

        Ok(())
    }

    fn init_sqlite(db_path: Option<PathBuf>) -> Result<Connection, SQLError> {
        let connection = match db_path {
            Some(mut path) => {
                if !path.is_dir() {
                    std::fs::create_dir_all(&path).ok();
                }

                path.push(FILENAME_DB);

                std::fs::remove_file(&path).ok();
                Connection::open(&path)?
            }
            None => Connection::open_in_memory()?,
        };

        let schema = include_str!("schema_stats.sql");

        let mut batch = Batch::new(&connection, schema);
        while let Some(mut stmt) = batch.next()? {
            stmt.execute([])?;
        }

        Ok(connection)
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::HashTrait;

    use super::*;

    #[test]
    fn test_timing_db() {
        let mut timing = Timing::new(None);

        assert!(timing.current_block.is_none());

        let block_hash = BlockHash::try_from_bytes(&vec![1; 32]).unwrap();
        timing
            .set_current_block(Some(block_hash.clone()), None)
            .unwrap();
        let block_id = timing.current_block.clone().unwrap().0;

        timing.set_current_block(Some(block_hash), None).unwrap();
        let same_block_id = timing.current_block.clone().unwrap().0;

        assert_eq!(block_id, same_block_id);

        timing
            .set_current_block(Some(BlockHash::try_from_bytes(&vec![2; 32]).unwrap()), None)
            .unwrap();
        let other_block_id = timing.current_block.clone().unwrap().0;

        assert_ne!(block_id, other_block_id);

        timing
            .insert_action(&Action {
                action_name: ActionKind::Mem,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(1.0),
                tezedge_time: Some(2.0),
            })
            .unwrap();

        timing.sync_block_stats().unwrap();
        timing.sync_global_stats(Some(1.0), Some(1.0)).unwrap();
    }

    #[test]
    fn test_actions_db() {
        let block_hash = BlockHash::try_from_bytes(&vec![1; 32]).unwrap();
        let context_hash = ContextHash::try_from_bytes(&vec![2; 32]).unwrap();

        TIMING_CHANNEL
            .send(TimingMessage::InitTiming { db_path: None })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::SetBlock {
                block_hash: Some(block_hash),
                start_at: None,
            })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Checkout {
                context_hash,
                irmin_time: Some(1.0),
                tezedge_time: Some(2.0),
            })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(1.0),
                tezedge_time: Some(2.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(5.0),
                tezedge_time: Some(6.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(50.0),
                tezedge_time: Some(60.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Mem,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(10.0),
                tezedge_time: Some(20.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(15.0),
                tezedge_time: Some(26.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: Some(150.0),
                tezedge_time: Some(260.0),
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Commit {
                irmin_time: Some(15.0),
                tezedge_time: Some(20.0),
            })
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
