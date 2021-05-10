// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crossbeam_channel::{unbounded, Receiver, Sender};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use once_cell::sync::Lazy;
use rusqlite::{named_params, Batch, Connection, Error as SQLError};
use serde::Serialize;
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};

pub const DB_PATH: &str = "context_stats.db";

pub fn set_block(rt: &OCamlRuntime, block_hash: OCamlRef<Option<OCamlBlockHash>>) {
    let block_hash: Option<BlockHash> = block_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::SetBlock(block_hash))
        .unwrap();
}

pub fn set_operation(rt: &OCamlRuntime, operation_hash: OCamlRef<Option<OCamlOperationHash>>) {
    let operation_hash: Option<OperationHash> = operation_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::SetOperation(operation_hash))
        .unwrap();
}

pub fn checkout(
    rt: &OCamlRuntime,
    context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let context_hash: ContextHash = context_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::Checkout {
            context_hash,
            irmin_time,
            tezedge_time,
        })
        .unwrap();
}

pub fn commit(
    _rt: &OCamlRuntime,
    _new_context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    TIMING_CHANNEL
        .send(TimingMessage::Commit {
            irmin_time,
            tezedge_time,
        })
        .unwrap();
}

pub fn context_action(
    rt: &OCamlRuntime,
    action_name: OCamlRef<String>,
    key: OCamlRef<OCamlList<String>>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    // TODO - Bruno: it is possible to avoid these conversions by borrowing the internal
    // &str directly. Since we want this function to add as little overhead as possible
    // investigate doing that later once everything is working properly.

    let action_name = rt.get(action_name);
    let action_name = match action_name.as_bytes() {
        b"mem" => ActionKind::Mem,
        b"find" => ActionKind::Find,
        b"find_tree" => ActionKind::FindTree,
        b"add" => ActionKind::Add,
        b"add_tree" => ActionKind::AddTree,
        b"remove" => ActionKind::Remove,
        _ => return,
    };

    let key: Vec<String> = key.to_rust(rt);

    let action = Action {
        action_name,
        key,
        irmin_time,
        tezedge_time,
    };

    TIMING_CHANNEL.send(TimingMessage::Action(action)).unwrap();
}

#[derive(Debug)]
enum ActionKind {
    Mem,
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
enum TimingMessage {
    SetBlock(Option<BlockHash>),
    SetOperation(Option<OperationHash>),
    Checkout {
        context_hash: ContextHash,
        irmin_time: f64,
        tezedge_time: f64,
    },
    Commit {
        irmin_time: f64,
        tezedge_time: f64,
    },
    Action(Action),
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

    fn add_time(&mut self, tezedge_time: f64) {
        let time = match tezedge_time {
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
        time.count = time.count.saturating_add(1);
        time.total_time += tezedge_time;
        time.max_time = time.max_time.max(tezedge_time);
    }
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActionStatsWithRange {
    pub root: String,
    pub mem: RangeStats,
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
}

struct Timing {
    current_block: Option<(HashId, BlockHash)>,
    current_operation: Option<(HashId, OperationHash)>,
    current_context: Option<(HashId, ContextHash)>,
    /// Number of actions in current block
    nactions: usize,
    /// Checkout time for the current block
    checkout_time: Option<(f64, f64)>,
    global_stats: HashMap<String, ActionStatsWithRange>,
    commit_stats: RangeStats,
    checkout_stats: RangeStats,
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
struct Action {
    action_name: ActionKind,
    key: Vec<String>,
    irmin_time: f64,
    tezedge_time: f64,
}

static TIMING_CHANNEL: Lazy<Sender<TimingMessage>> = Lazy::new(|| {
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
    let mut timing = Timing::new();

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
    fn new() -> Timing {
        let sql = Self::init_sqlite().unwrap();

        Timing {
            current_block: None,
            current_operation: None,
            current_context: None,
            nactions: 0,
            global_stats: HashMap::default(),
            commit_stats: Default::default(),
            checkout_stats: Default::default(),
            sql,
            checkout_time: None,
        }
    }

    fn process_msg(&mut self, msg: TimingMessage) -> Result<(), SQLError> {
        match msg {
            TimingMessage::SetBlock(block_hash) => self.set_current_block(block_hash),
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
        }
    }

    fn set_current_block(&mut self, block_hash: Option<BlockHash>) -> Result<(), SQLError> {
        Self::set_current(&self.sql, block_hash, &mut self.current_block, "blocks")?;

        // Reset context and operation
        self.current_context = None;
        self.current_operation = None;
        self.checkout_time = None;
        self.nactions = 0;

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
        irmin_time: f64,
        tezedge_time: f64,
    ) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.checkout_stats.add_time(tezedge_time);
        self.set_current_context(context_hash)?;
        self.checkout_time = Some((irmin_time, tezedge_time));

        Ok(())
    }

    fn insert_commit(&mut self, irmin_time: f64, tezedge_time: f64) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        self.commit_stats.add_time(tezedge_time);
        self.update_global_stats(irmin_time, tezedge_time)
    }

    fn insert_action(&mut self, action: &Action) -> Result<(), SQLError> {
        if self.current_block.is_none() {
            return Ok(());
        }

        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());
        let operation_id = self.current_operation.as_ref().map(|(id, _)| id.as_str());
        let context_id = self.current_context.as_ref().map(|(id, _)| id.as_str());
        let action_name = action.action_name.to_str();

        let (root, key) = if action.key.is_empty() {
            (None, None)
        } else {
            let root = action.key[0].as_str();
            let key = action.key.join("/");

            (Some(root), Some(key))
        };

        let mut stmt = self.sql.prepare_cached(
            "
        INSERT INTO actions
          (name, key_root, key, irmin_time, tezedge_time, block_id, operation_id, context_id)
        VALUES
          (:name, :key_root, :key, :irmin_time, :tezedge_time, :block_id, :operation_id, :context_id);
            "
        )?;

        stmt.execute(named_params! {
            ":name": action_name,
            ":key_root": &root,
            ":key": &key,
            ":irmin_time": &action.irmin_time,
            ":tezedge_time": &action.tezedge_time,
            ":block_id": block_id,
            ":operation_id": operation_id,
            ":context_id": context_id
        })?;

        self.nactions = self
            .nactions
            .checked_add(1)
            .expect("actions count overflowed");

        let root = match root {
            Some(root) => root,
            None => return Ok(()),
        };
        let tezedge_time: f64 = action.tezedge_time;

        let entry = match self.global_stats.get_mut(root) {
            Some(entry) => entry,
            None => {
                let mut stats = ActionStatsWithRange::default();
                stats.root = root.to_string();
                self.global_stats.insert(root.to_string(), stats);
                self.global_stats.get_mut(root).unwrap()
            }
        };

        let action_stats = match action.action_name {
            ActionKind::Mem => &mut entry.mem,
            ActionKind::Find => &mut entry.find,
            ActionKind::FindTree => &mut entry.find_tree,
            ActionKind::Add => &mut entry.add,
            ActionKind::AddTree => &mut entry.add_tree,
            ActionKind::Remove => &mut entry.remove,
        };

        action_stats.add_time(tezedge_time);

        Ok(())
    }

    // Compute stats for the current block and global ones
    fn update_global_stats(
        &mut self,
        commit_time_irmin: f64,
        commit_time_tezedge: f64,
    ) -> Result<(), SQLError> {
        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());

        self.sql.execute(
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
            named_params! {
                ":actions_count": &self.nactions,
                ":checkout_time_irmin": &self.checkout_time.as_ref().map(|(irmin, _)| irmin),
                ":checkout_time_tezedge": &self.checkout_time.as_ref().map(|(_, tezedge)| tezedge),
                ":commit_time_irmin": &commit_time_irmin,
                ":commit_time_tezedge": &commit_time_tezedge,
                ":block_id": block_id
            },
        )?;

        for action in self.global_stats.values_mut() {
            action.compute_mean();
        }

        for (root, action_stats) in self.global_stats.iter() {
            let root = root.as_str();

            self.insert_action_stats(root, "mem", &action_stats.mem)?;
            self.insert_action_stats(root, "find", &action_stats.find)?;
            self.insert_action_stats(root, "find_tree", &action_stats.find_tree)?;
            self.insert_action_stats(root, "add", &action_stats.add)?;
            self.insert_action_stats(root, "add_tree", &action_stats.add_tree)?;
            self.insert_action_stats(root, "remove", &action_stats.remove)?;
        }

        self.checkout_stats.compute_mean();
        self.commit_stats.compute_mean();
        self.insert_action_stats("commit", "commit", &self.commit_stats)?;
        self.insert_action_stats("checkout", "checkout", &self.checkout_stats)?;

        Ok(())
    }

    fn insert_action_stats(
        &self,
        root: &str,
        action_name: &str,
        range_stats: &RangeStats,
    ) -> Result<(), SQLError> {
        let mut query = self.sql.prepare_cached(
            "
        INSERT OR IGNORE INTO global_action_stats
          (root, action_name)
        VALUES
          (:root, :action_name)
            ",
        )?;

        query.execute(named_params! {
            ":root": root,
            ":action_name": action_name,
        })?;

        let mut query = self.sql.prepare_cached(
            "
        UPDATE
          global_action_stats
        SET
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
          root = :root AND action_name = :action_name;
        ",
        )?;

        query.execute(
            named_params! {
                ":root": root,
                ":action_name": action_name,
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

    fn init_sqlite() -> Result<Connection, SQLError> {
        std::fs::remove_file(DB_PATH).ok();

        let connection = Connection::open(DB_PATH)?;
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
        let mut timing = Timing::new();

        assert!(timing.current_block.is_none());

        let block_hash = BlockHash::try_from_bytes(&vec![1; 32]).unwrap();
        timing.set_current_block(Some(block_hash.clone())).unwrap();
        let block_id = timing.current_block.clone().unwrap().0;

        timing.set_current_block(Some(block_hash)).unwrap();
        let same_block_id = timing.current_block.clone().unwrap().0;

        assert_eq!(block_id, same_block_id);

        timing
            .set_current_block(Some(BlockHash::try_from_bytes(&vec![2; 32]).unwrap()))
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
                irmin_time: 1.0,
                tezedge_time: 2.0,
            })
            .unwrap();

        timing.update_global_stats(1.0, 1.0).unwrap();
    }

    #[test]
    fn test_actions_db() {
        let block_hash = BlockHash::try_from_bytes(&vec![1; 32]).unwrap();
        let context_hash = ContextHash::try_from_bytes(&vec![2; 32]).unwrap();

        TIMING_CHANNEL
            .send(TimingMessage::SetBlock(Some(block_hash)))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Checkout {
                context_hash,
                irmin_time: 1.0,
                tezedge_time: 2.0,
            })
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 1.0,
                tezedge_time: 2.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 5.0,
                tezedge_time: 6.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Find,
                key: vec!["a", "b", "c"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 50.0,
                tezedge_time: 60.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Mem,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 10.0,
                tezedge_time: 20.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 15.0,
                tezedge_time: 26.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Action(Action {
                action_name: ActionKind::Add,
                key: vec!["m", "n", "o"]
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                irmin_time: 150.0,
                tezedge_time: 260.0,
            }))
            .unwrap();
        TIMING_CHANNEL
            .send(TimingMessage::Commit {
                irmin_time: 15.0,
                tezedge_time: 20.0,
            })
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
