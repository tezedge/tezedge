// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crossbeam_channel::{unbounded, Receiver, Sender};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use once_cell::sync::Lazy;
use rusqlite::{named_params, Batch, Connection, Error as SQLError};
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};

const DB_PATH: &str = "context_stats.db";

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

struct Timing {
    current_block: Option<(HashId, BlockHash)>,
    current_operation: Option<(HashId, OperationHash)>,
    current_context: Option<(HashId, ContextHash)>,
    /// Number of actions in current block
    nactions: usize,
    /// Checkout time for the current block
    checkout_time: Option<(f64, f64)>,
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

    pub fn hash_to_string(hash: &[u8]) -> String {
        const HEXCHARS: &[u8] = b"0123456789abcdef";

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
        let hash_string = Self::hash_to_string(hash.as_ref());

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string)? {
            current.replace((id.to_string(), hash));
            return Ok(());
        };

        sql.execute(
            &format!(
                "INSERT INTO {table} (hash) VALUES (?1);",
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
        self.set_current_context(context_hash)?;
        self.checkout_time = Some((irmin_time, tezedge_time));

        Ok(())
    }

    fn insert_commit(&mut self, irmin_time: f64, tezedge_time: f64) -> Result<(), SQLError> {
        self.update_global_stats(irmin_time, tezedge_time)
    }

    fn insert_action(&mut self, action: &Action) -> Result<(), SQLError> {
        let block_id = self.current_block.as_ref().map(|(id, _)| id.as_str());
        let operation_id = self.current_operation.as_ref().map(|(id, _)| id.as_str());
        let context_id = self.current_context.as_ref().map(|(id, _)| id.as_str());

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
            ":name": &action.name,
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

        Ok(())
    }

    fn init_sqlite() -> Result<Connection, SQLError> {
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
                name: "some_action".to_string(),
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
                name: "add".to_string(),
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
                name: "find".to_string(),
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
                name: "find".to_string(),
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
                name: "mem".to_string(),
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
                name: "add".to_string(),
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
                name: "add".to_string(),
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
