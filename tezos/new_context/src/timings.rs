// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use crossbeam_channel::{unbounded, Receiver, Sender};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use once_cell::sync::Lazy;
use rusqlite::{Batch, Connection, Error as SQLError, named_params};
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};

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
    let name: String = action_name.to_rust(rt);
    let key: Vec<String> = key.to_rust(rt);
    // let _ = (action_name, key, irmin_time, tezedge_time);
    // TODO

    let action = Action {
        name,
        key,
        irmin_time,
        tezedge_time,
    };

    TIMING_CHANNEL.send(TimingMessage::Action(action)).unwrap();
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

#[derive(Default)]
struct Times {
    tezedge_times: Vec<f64>,
    irmin_times: Vec<f64>,
}

impl<'a> std::iter::Sum<&'a Times> for Times {
    fn sum<I: Iterator<Item = &'a Times>>(iter: I) -> Self {
        let mut times = Times::default();

        for elem in iter {
            times.tezedge_times.extend_from_slice(&elem.tezedge_times);
            times.irmin_times.extend_from_slice(&elem.irmin_times);
        }

        times
    }
}

struct Timing {
    current_block: Option<(HashId, BlockHash)>,
    current_operation: Option<(HashId, OperationHash)>,
    current_context: Option<(HashId, ContextHash)>,
    /// Total number of actions since start
    nactions: usize,
    /// Stats for all commits since start
    commits: Times,
    /// Stats for all checkouts since start
    checkouts: Times,
    /// Times in the current block, by action name
    times_in_current_block_by_name: HashMap<String, Times>,
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
    name: String,
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

impl Times {
    fn add(&mut self, irmin_time: f64, tezedge_time: f64) {
        self.irmin_times.push(irmin_time);
        self.tezedge_times.push(tezedge_time);
    }

    fn sum(&self) -> (f64, f64) {
        let irmin = self.irmin_times.iter().sum();
        let tezedge = self.tezedge_times.iter().sum();

        (irmin, tezedge)
    }
}

fn start_timing(recv: Receiver<TimingMessage>) {
    let mut timing = Timing::new();

    for msg in recv {
        timing.process_msg(msg).unwrap();
    }
}

impl Timing {
    fn new() -> Timing {
        let sql = Self::init_sqlite().unwrap();

        Timing {
            current_block: None,
            current_operation: None,
            current_context: None,
            nactions: 0,
            commits: Times::default(),
            checkouts: Times::default(),
            times_in_current_block_by_name: HashMap::default(),
            sql,
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

    pub fn hash_to_string(hash: &[u8]) -> String {
        const HEXCHARS: &[u8] = b"0123456789ABCDEF";

        let mut s = String::with_capacity(62);
        for byte in hash {
            s.push(HEXCHARS[*byte as usize >> 4] as char);
            s.push(HEXCHARS[*byte as usize & 0xF] as char);
        }
        s
    }

    fn set_current_block(&mut self, block_hash: Option<BlockHash>) -> Result<(), SQLError> {
        Self::set_current(&self.sql, block_hash, &mut self.current_block, "blocks")?;

        // Reset context and operation
        self.current_context = None;
        self.current_operation = None;

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
        let hash_string = Self::hash_to_string(hash.as_ref());

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string)? {
            current.replace((id.to_string(), hash));
            return Ok(());
        };

        sql.execute(
            &format!("INSERT INTO {table} (hash) VALUES (?1);", table = table_name),
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
        let mut stmt = sql.prepare(&format!("SELECT id FROM {table} WHERE hash = ?1;", table = table_name))?;
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
        self.checkouts.add(irmin_time, tezedge_time);
        self.set_current_context(context_hash)?;
        self.insert_action(&Action {
            name: "checkout".to_string(),
            key: vec![],
            irmin_time,
            tezedge_time,
        })
    }

    fn insert_commit(&mut self, irmin_time: f64, tezedge_time: f64) -> Result<(), SQLError> {
        self.commits.add(irmin_time, tezedge_time);
        self.insert_action(&Action {
            name: "commit".to_string(),
            key: vec![],
            irmin_time,
            tezedge_time,
        })?;
        self.update_global_stats()
    }

    fn insert_action(&mut self, action: &Action) -> Result<(), SQLError> {
        let block_id = self
            .current_block
            .as_ref()
            .map(|(id, _)| id.as_str())
            .unwrap_or("NULL");
        let operation_id = self
            .current_operation
            .as_ref()
            .map(|(id, _)| id.as_str())
            .unwrap_or("NULL");
        let context_id = self
            .current_context
            .as_ref()
            .map(|(id, _)| id.as_str())
            .unwrap_or("NULL");

        let key = if action.key.is_empty() {
            "NULL".to_string()
        } else {
            action.key.join("/")
        };

        self.sql.execute(
            "
        INSERT INTO actions
          (name, key, irmin_time, tezedge_time, block_id, operation_id, context_id)
        VALUES
          (:name, :key, :irmin_time, :tezedge_time, :block_id, :operation_id, :context_id);
            ",
            named_params!{
                ":name": &action.name,
                ":key": &key,
                ":irmin_time": &action.irmin_time,
                ":tezedge_time": &action.tezedge_time,
                ":block_id": block_id,
                ":operation_id": operation_id,
                ":context_id": context_id
            }
        )?;

        self.nactions = self
            .nactions
            .checked_add(1)
            .expect("actions count overflowed");

        self.times_in_current_block_by_name
            .entry(action.name.clone())
            .or_default()
            .add(action.irmin_time, action.tezedge_time);

        Ok(())
    }

    // Compute stats for the current block and global ones
    fn update_global_stats(&mut self) -> Result<(), SQLError> {
        let block_id = self
            .current_block
            .as_ref()
            .map(|(id, _)| id.as_str())
            .unwrap_or("NULL");

        // Compute global stats

        let tezedge_commits_total = self.commits.tezedge_times.iter().sum::<f64>();
        let tezedge_commits_mean = tezedge_commits_total / self.commits.tezedge_times.len() as f64;
        let tezedge_commits_max = self
            .commits
            .tezedge_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        let irmin_commits_total = self.commits.irmin_times.iter().sum::<f64>();
        let irmin_commits_mean = irmin_commits_total / self.commits.irmin_times.len() as f64;
        let irmin_commits_max = self
            .commits
            .irmin_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        let tezedge_checkouts_total = self.checkouts.tezedge_times.iter().sum::<f64>();
        let tezedge_checkouts_mean =
            tezedge_checkouts_total / self.checkouts.tezedge_times.len() as f64;
        let tezedge_checkouts_max = self
            .checkouts
            .tezedge_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        let irmin_checkouts_total = self.checkouts.irmin_times.iter().sum::<f64>();
        let irmin_checkouts_mean = irmin_checkouts_total / self.checkouts.irmin_times.len() as f64;
        let irmin_checkouts_max = self
            .checkouts
            .irmin_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        // Compute stats for the current block, by action name

        for (name, times) in &self.times_in_current_block_by_name {
            let (time_irmin, time_tezedge) = times.sum();

            self.sql.execute(
                "
             INSERT INTO block_details
               (block_id, action_name, irmin_time, tezedge_time)
             VALUES
               (:block_id, :action_name, :time_irmin, :time_tezedge);
                ",
                named_params! {
                    ":block_id": &block_id,
                    ":action_name": name,
                    ":time_irmin": time_irmin.max(0.0),
                    ":time_tezedge": time_tezedge.max(0.0),
                }
            )?;
        }

        // Compute stats for all actions in the current block

        let times: Times = self.times_in_current_block_by_name.values().sum();

        let tezedge_time_total = times.tezedge_times.iter().sum::<f64>();
        let tezedge_time_mean = tezedge_time_total / times.tezedge_times.len() as f64;
        let tezedge_time_max = times
            .tezedge_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        let irmin_time_total = times.irmin_times.iter().sum::<f64>();
        let irmin_time_mean = irmin_time_total / times.irmin_times.len() as f64;
        let irmin_time_max = times
            .irmin_times
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);

        self.sql.execute(
            "
        UPDATE
          blocks
        SET
          tezedge_time_max = :tezedge_time_max,
          tezedge_time_mean = :tezedge_time_mean,
          tezedge_time_total = :tezedge_time_total,
          irmin_time_max = :irmin_time_max,
          irmin_time_mean = :irmin_time_mean,
          irmin_time_total = :irmin_time_total
        WHERE
          id = :block_id;
            ",
            named_params! {
                ":tezedge_time_max": tezedge_time_max.max(0.0),
                ":tezedge_time_mean": tezedge_time_mean.max(0.0),
                ":tezedge_time_total": tezedge_time_total.max(0.0),
                ":irmin_time_max": irmin_time_max.max(0.0),
                ":irmin_time_mean": irmin_time_mean.max(0.0),
                ":irmin_time_total": irmin_time_total.max(0.0),
                ":block_id": block_id
            }
        )?;

        self.sql.execute(
            "
        UPDATE
          global_stats
        SET
          actions_count = :actions_count,
          tezedge_checkouts_max = :tezedge_checkouts_max,
          tezedge_checkouts_mean = :tezedge_checkouts_mean,
          tezedge_checkouts_total = :tezedge_checkouts_total,
          irmin_checkouts_max = :irmin_checkouts_max,
          irmin_checkouts_mean = :irmin_checkouts_mean,
          irmin_checkouts_total = :irmin_checkouts_total,
          tezedge_commits_max = :tezedge_commits_max,
          tezedge_commits_mean = :tezedge_commits_mean,
          tezedge_commits_total = :tezedge_commits_total,
          irmin_commits_max = :irmin_commits_max,
          irmin_commits_mean = :irmin_commits_mean,
          irmin_commits_total = :irmin_commits_total
        WHERE
          id = 0;
            ",
            named_params! {
                ":actions_count": &self.nactions,
                ":tezedge_checkouts_max": tezedge_checkouts_max.max(0.0),
                ":tezedge_checkouts_mean": tezedge_checkouts_mean.max(0.0),
                ":tezedge_checkouts_total": tezedge_checkouts_total.max(0.0),
                ":irmin_checkouts_max": irmin_checkouts_max.max(0.0),
                ":irmin_checkouts_mean": irmin_checkouts_mean.max(0.0),
                ":irmin_checkouts_total": irmin_checkouts_total.max(0.0),
                ":tezedge_commits_max": tezedge_commits_max.max(0.0),
                ":tezedge_commits_mean": tezedge_commits_mean.max(0.0),
                ":tezedge_commits_total": tezedge_commits_total.max(0.0),
                ":irmin_commits_max": irmin_commits_max.max(0.0),
                ":irmin_commits_mean": irmin_commits_mean.max(0.0),
                ":irmin_commits_total": irmin_commits_total.max(0.0)
            }
        )?;

        // Reset block stats
        self.times_in_current_block_by_name = HashMap::new();

        Ok(())
    }

    fn init_sqlite() -> Result<Connection, SQLError> {
        let connection = Connection::open("context_stats.db")?;

        let queries =
                "
        PRAGMA foreign_keys = ON;
        PRAGMA synchronous = OFF;
        CREATE TABLE IF NOT EXISTS blocks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            hash TEXT UNIQUE,
            tezedge_time_max REAL,
            tezedge_time_mean REAL,
            tezedge_time_total REAL,
            irmin_time_max REAL,
            irmin_time_mean REAL,
            irmin_time_total REAL
        );
        CREATE TABLE IF NOT EXISTS operations (id INTEGER PRIMARY KEY AUTOINCREMENT, hash TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS contexts (id INTEGER PRIMARY KEY AUTOINCREMENT, hash TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS block_details (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action_name TEXT NOT NULL,
            irmin_time REAL NOT NULL,
            tezedge_time REAL NOT NULL,
            block_id INTEGER DEFAULT NULL,
            FOREIGN KEY(block_id) REFERENCES blocks(id)
        );
        CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            key TEXT,
            irmin_time REAL,
            tezedge_time REAL,
            block_id INTEGER DEFAULT NULL,
            operation_id INTEGER DEFAULT NULL,
            context_id INTEGER DEFAULT NULL,
            FOREIGN KEY(block_id) REFERENCES blocks(id),
            FOREIGN KEY(operation_id) REFERENCES operations(id),
            FOREIGN KEY(context_id) REFERENCES contexts(id)
        );
        CREATE TABLE IF NOT EXISTS global_stats (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            actions_count INTEGER,
            tezedge_checkouts_max REAL,
            tezedge_checkouts_mean REAL,
            tezedge_checkouts_total REAL,
            irmin_checkouts_max REAL,
            irmin_checkouts_mean REAL,
            irmin_checkouts_total REAL,
            tezedge_commits_max REAL,
            tezedge_commits_mean REAL,
            tezedge_commits_total REAL,
            irmin_commits_max REAL,
            irmin_commits_mean REAL,
            irmin_commits_total REAL
        );
        INSERT INTO global_stats (id) VALUES (0) ON CONFLICT DO NOTHING;
                ";

        let mut batch = Batch::new(&connection, queries);
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

        timing.insert_action(&Action {
            name: "some_action".to_string(),
            key: vec!["a", "b", "c"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            irmin_time: 1.0,
            tezedge_time: 2.0,
        }).unwrap();

        timing.update_global_stats().unwrap();
    }
}
