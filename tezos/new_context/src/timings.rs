// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crossbeam_channel::{unbounded, Receiver, Sender};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use once_cell::sync::Lazy;
use sqlite::Value::Integer;
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
    rt: &OCamlRuntime,
    new_context_hash: OCamlRef<OCamlContextHash>,
    irmin_time: f64,
    tezedge_time: f64,
) {
    let new_context_hash: ContextHash = new_context_hash.to_rust(rt);

    TIMING_CHANNEL
        .send(TimingMessage::Commit {
            new_context_hash,
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

enum TimingMessage {
    SetBlock(Option<BlockHash>),
    SetOperation(Option<OperationHash>),
    Checkout {
        context_hash: ContextHash,
        irmin_time: f64,
        tezedge_time: f64,
    },
    Commit {
        new_context_hash: ContextHash,
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
    sql: sqlite::Connection,
}

impl std::fmt::Debug for Timing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Timing")
            .field("current_block", &self.current_block)
            .field("current_operation", &self.current_operation)
            .finish()
    }
}

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
        timing.process_msg(msg);
    }
}

impl Timing {
    fn new() -> Timing {
        let sql = Self::init_sqlite();

        Timing {
            current_block: None,
            current_operation: None,
            sql,
        }
    }

    fn process_msg(&mut self, msg: TimingMessage) {
        match msg {
            TimingMessage::SetBlock(block_hash) => {
                self.set_current_block(block_hash);
            }
            TimingMessage::SetOperation(operation_hash) => {
                self.set_current_operation(operation_hash);
            }
            TimingMessage::Action(action) => {
                self.insert_action(&action);
            }
            TimingMessage::Checkout {
                context_hash,
                irmin_time,
                tezedge_time,
            } => {}
            TimingMessage::Commit {
                new_context_hash,
                irmin_time,
                tezedge_time,
            } => {}
        }
    }

    pub fn hash_to_string(hash: &[u8]) -> String {
        let mut s = String::with_capacity(32);
        for byte in hash {
            s.push_str(&format!("{:X}", byte));
        }
        s
    }

    fn set_current_block(&mut self, block_hash: Option<BlockHash>) {
        Self::set_current(&self.sql, block_hash, &mut self.current_block, "blocks");
    }

    fn set_current_operation(&mut self, operation_hash: Option<OperationHash>) {
        Self::set_current(
            &self.sql,
            operation_hash,
            &mut self.current_operation,
            "operations",
        );
    }

    fn set_current<T>(
        sql: &sqlite::Connection,
        hash: Option<T>,
        current: &mut Option<(String, T)>,
        table_name: &str,
    ) where
        T: Eq,
        T: AsRef<Vec<u8>>,
    {
        match (hash.as_ref(), current.as_ref()) {
            (None, _) => {
                *current = None;
                return;
            }
            (Some(hash), Some((_, current_hash))) if hash == current_hash => {
                return;
            }
            _ => {}
        };

        let hash = hash.unwrap();
        let hash_string = Self::hash_to_string(hash.as_ref());

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string) {
            current.replace((id.to_string(), hash));
            return;
        };

        let insert_query = format!(
            "INSERT INTO {table} (hash) VALUES ('{hash}');",
            table = table_name,
            hash = hash_string
        );

        sql.execute(insert_query).unwrap();

        if let Some(id) = Self::get_id_on_table(sql, table_name, &hash_string) {
            current.replace((id.to_string(), hash));
        };
    }

    fn get_id_on_table(
        sql: &sqlite::Connection,
        table_name: &str,
        hash_string: &str,
    ) -> Option<i64> {
        let query = format!(
            "SELECT id FROM {table} WHERE hash = '{hash}'",
            table = table_name,
            hash = hash_string
        );

        let mut cursor = sql.prepare(query).unwrap().into_cursor();

        if let Ok(Some([Integer(id), ..])) = cursor.next() {
            return Some(*id);
        };

        None
    }

    fn insert_action(&self, action: &Action) {
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

        let query = format!(
            "
        INSERT INTO actions
          (name, key, irmin_time, tezedge_time, block_id, operation_id)
        VALUES
          ('{name}', '{key}', {irmin_time}, {tezedge_time}, {block_id}, {operation_id});
            ",
            name = &action.name,
            key = &action.key.join("/"),
            irmin_time = &action.irmin_time,
            tezedge_time = &action.tezedge_time,
            block_id = block_id,
            operation_id = operation_id,
        );

        self.sql.execute(query).unwrap();
    }

    fn init_sqlite() -> sqlite::Connection {
        let connection = sqlite::open("context_timing.sql").unwrap();

        connection
            .execute(
                "
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS blocks (id INTEGER PRIMARY KEY AUTOINCREMENT, hash TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS operations (id INTEGER PRIMARY KEY AUTOINCREMENT, hash TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS context (id INTEGER PRIMARY KEY AUTOINCREMENT, hash TEXT UNIQUE);
        CREATE TABLE IF NOT EXISTS actions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT,
            key TEXT,
            irmin_time REAL,
            tezedge_time REAL,
            block_id INTEGER DEFAULT NULL,
            operation_id INTEGER DEFAULT NULL,
            FOREIGN KEY(block_id) REFERENCES blocks(id),
            FOREIGN KEY(operation_id) REFERENCES operations(id)
        );
                ",
            )
            .unwrap();

        connection
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::HashTrait;

    use super::*;

    #[test]
    fn test_init_db() {
        let mut timing = Timing::new();

        assert!(timing.current_block.is_none());

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![1; 32]).unwrap()));
        let block_id = timing.current_block.clone().unwrap().0;

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![1; 32]).unwrap()));
        let same_block_id = timing.current_block.clone().unwrap().0;

        assert_eq!(block_id, same_block_id);

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![2; 32]).unwrap()));
        let other_block_id = timing.current_block.clone().unwrap().0;

        assert_ne!(block_id, other_block_id);

        timing.insert_action(&Action {
            name: "action_name".to_string(),
            key: vec!["a", "b", "c"]
                .iter()
                .map(ToString::to_string)
                .collect(),
            irmin_time: 1.0,
            tezedge_time: 2.0,
        });
    }
}
