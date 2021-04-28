// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crossbeam_channel::{Receiver, Sender, unbounded};
use crypto::hash::{BlockHash, ContextHash, OperationHash};
use ocaml_interop::*;
use sqlite::{Cursor, Value::Integer};
use tezos_api::ocaml_conv::{OCamlBlockHash, OCamlContextHash, OCamlOperationHash};
use once_cell::sync::Lazy;

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

    TIMING_CHANNEL
        .send(TimingMessage::Action(action))
        .unwrap();
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

struct Timing {
    current_block: Option<(i64, BlockHash)>,
    current_operation: Option<(i64, OperationHash)>,
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

// struct Block {
//     hash: BlockHash,
//     operations: Vec<Operation>
// }

// struct Operation {
//     hash: OperationHash,
//     actions: Vec<Action>
// }

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
        let connection = sqlite::open("context_timing.sql").unwrap();

        Timing {
            current_block: None,
            current_operation: None,
            sql: connection,
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
            TimingMessage::Checkout { context_hash, irmin_time, tezedge_time } => {

            }
            TimingMessage::Commit { new_context_hash, irmin_time, tezedge_time } => {

            }
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
        let block_hash = match (block_hash, self.current_block.as_ref()) {
            (None, _) => {
                self.current_block = None;
                return;
            }
            (Some(block_hash), None) => {
                block_hash
            }
            (Some(block_hash), Some((_, current_block_hash))) => {
                if &block_hash == current_block_hash {
                    return;
                }
                block_hash
            }
        };

        let block_hash_string = Self::hash_to_string(block_hash.as_ref());

        if let Some(id) = self.get_block_id(&block_hash_string) {
            self.current_block = Some((id, block_hash));
            return;
        };

        let insert_query = format!(
            "INSERT INTO blocks (hash) VALUES ('{}');",
            block_hash_string
        );

        self.sql
            .execute(insert_query)
            .unwrap();

        if let Some(id) = self.get_block_id(&block_hash_string) {
            self.current_block = Some((id, block_hash));
        };
    }

    fn set_current_operation(&mut self, operation_hash: Option<OperationHash>) {
        let operation_hash = match (operation_hash, self.current_operation.as_ref()) {
            (None, _) => {
                self.current_block = None;
                return;
            }
            (Some(operation_hash), None) => {
                operation_hash
            }
            (Some(operation_hash), Some((_, current_operation_hash))) => {
                if &operation_hash == current_operation_hash {
                    return;
                }
                operation_hash
            }
        };

        let block_hash_string = Self::hash_to_string(operation_hash.as_ref());

        if let Some(id) = self.get_operation_id(&block_hash_string) {
            self.current_operation = Some((id, operation_hash));
            return;
        };

        let insert_query = format!(
            "INSERT INTO operations (hash) VALUES ('{}');",
            block_hash_string
        );

        self.sql
            .execute(insert_query)
            .unwrap();

        if let Some(id) = self.get_operation_id(&block_hash_string) {
            self.current_operation = Some((id, operation_hash));
        };
    }

    fn get_block_id(&self, block_hash_string: &str) -> Option<i64> {
        let mut cursor = self.make_cursor(
            format!(
                "SELECT id FROM blocks WHERE hash = '{}'",
                block_hash_string
            )
        );

        if let Some([Integer(id), ..]) = cursor.next().unwrap() {
            return Some(*id)
        };

        None
    }

    fn get_operation_id(&self, block_hash_string: &str) -> Option<i64> {
        let mut cursor = self.make_cursor(
            format!(
                "SELECT id FROM operations WHERE hash = '{}'",
                block_hash_string
            )
        );

        if let Some([Integer(id), ..]) = cursor.next().unwrap() {
            return Some(*id)
        };

        None
    }

    fn make_cursor<T: AsRef<str>>(&self, query: T) -> Cursor {
        self.sql
            .prepare(query)
            .unwrap()
            .into_cursor()
    }

    fn insert_action(&self, action: &Action) {
        let block_id = self.current_block.as_ref().map(|(id, _)| id.to_string()).unwrap_or_else(|| "NULL".to_string());
        let operation_id = self.current_operation.as_ref().map(|(id, _)| id.to_string()).unwrap_or_else(|| "NULL".to_string());

        let query = format!(
                "
        INSERT INTO actions
          (name, key, irmin_time, tezedge_time, block_id, operation_id)
        VALUES
          ('{name}', '{key}', {irmin_time}, {tezedge_time}, {block_id}, {operation_id});",
            name = &action.name,
            key = &action.key.join("/"),
            irmin_time = &action.irmin_time,
            tezedge_time = &action.tezedge_time,
            block_id = block_id,
            operation_id = operation_id,
        );

        self.sql
            .execute(query)
            .unwrap();
    }

    fn init_sqlite(&mut self) {
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

        // connection
        //     .execute(
        //         "
        // INSERT INTO blocks (id, hash) VALUES (1, 'abc');
        // INSERT INTO actions (name, key, block_id) VALUES ('hello', 'abc', 1);
        //         ",
        //     )
        //     .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::HashTrait;

    use super::*;

    #[test]
    fn test_init_db() {
        let mut timing = Timing::new();

        timing.init_sqlite();
        assert!(timing.current_block.is_none());

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![1; 32]).unwrap()));
        let block_id = timing.current_block.as_ref().unwrap().0;

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![1; 32]).unwrap()));
        let same_block_id = timing.current_block.as_ref().unwrap().0;

        assert_eq!(block_id, same_block_id);

        timing.set_current_block(Some(BlockHash::try_from_bytes(&vec![2; 32]).unwrap()));
        let other_block_id = timing.current_block.as_ref().unwrap().0;

        assert_ne!(block_id, other_block_id);

        println!("LA {:?}", timing);
        // timing.set_current_operation(Some("abc"));

        timing.insert_action(&Action {
            name: "action_name".to_string(),
            key: vec!["a", "b", "c"].iter().map(ToString::to_string).collect(),
            irmin_time: 1.0,
            tezedge_time: 2.0,
        });
    }
}
