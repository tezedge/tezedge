// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use crossbeam_channel::{Receiver, RecvError};

use crate::{chunks::ChunkedVec, kv_store::HashId, serialize::in_memory::iter_hash_ids};

use tezos_spsc::Producer;

pub(crate) const PRESERVE_CYCLE_COUNT: usize = 8;

/// Used for statistics
///
/// Number of items in `GCThread::pending`.
pub(crate) static GC_PENDING_HASHIDS: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct GCThread {
    pub(crate) cycles: Cycles,
    pub(crate) free_ids: Producer<HashId>,
    pub(crate) recv: Receiver<Command>,
    pub(crate) pending: Vec<HashId>,
}

pub(crate) enum Command {
    StartNewCycle {
        values_in_cycle: BTreeMap<HashId, Arc<[u8]>>,
        new_ids: ChunkedVec<HashId>,
    },
    MarkReused {
        reused: Vec<HashId>,
    },
    Close,
}

pub(crate) struct Cycles {
    list: VecDeque<BTreeMap<HashId, Arc<[u8]>>>,
}

impl Default for Cycles {
    fn default() -> Self {
        let mut list = VecDeque::with_capacity(PRESERVE_CYCLE_COUNT);

        for _ in 0..PRESERVE_CYCLE_COUNT {
            list.push_back(Default::default());
        }

        Self { list }
    }
}

impl Cycles {
    fn move_to_last_cycle(&mut self, hash_id: HashId) -> Option<Arc<[u8]>> {
        let mut value = None;

        for store in self.list.iter_mut().take(PRESERVE_CYCLE_COUNT - 1) {
            // if let Some(v) = store.get(&hash_id) {
            //     println!("SOME {:?}", v.is_some());
            // }

            if let Some(item) = store.remove(&hash_id) {
                // println!("FOUND SOME");
                value = Some(item);
            };
        }

        let value = value?;

        // let value = match value {
        //     Some(value) => value,
        //     None => return None,
        // };

        // if value.is_none() {
        //     return None;
        // }

        // value.as_ref()?;

        if let Some(last_cycle) = self.list.back_mut() {
            last_cycle.insert(hash_id, Arc::clone(&value));
            // if let Some(v) = last_cycle.get(&hash_id) {
            //     println!("SOME1 {:?}", v.is_some());
            // } else {
            //     println!("NONE");
            // }
        } else {
            eprintln!("GC: Failed to insert value in Cycles")
        }

        Some(value)
    }

    fn roll(&mut self, new_cycle: BTreeMap<HashId, Arc<[u8]>>) -> Vec<HashId> {
        let unused = self.list.pop_front().unwrap_or_default();
        self.list.push_back(new_cycle);

        let mut vec = Vec::with_capacity(unused.len());
        for id in unused.keys() {
            vec.push(*id);
        }
        vec
    }
}

impl GCThread {
    pub(crate) fn run(mut self) {
        loop {
            let msg = self.recv.recv();

            self.debug(&msg);

            match msg {
                Ok(Command::StartNewCycle {
                    values_in_cycle,
                    new_ids,
                }) => self.start_new_cycle(values_in_cycle, new_ids),
                Ok(Command::MarkReused { reused }) => self.mark_reused(reused),
                Ok(Command::Close) => {
                    println!("GC received Command::Close");
                    break;
                }
                Err(e) => {
                    eprintln!("GC channel is closed {:?}", e);
                    break;
                }
            }
        }
        eprintln!("GC exited");
    }

    fn debug(&self, msg: &Result<Command, RecvError>) {
        let msg = match msg {
            Ok(Command::StartNewCycle {
                values_in_cycle,
                new_ids,
            }) => format!(
                "START_NEW_CYCLE VALUES_IN_CYCLE={:?} NEW_IDS={:?}",
                values_in_cycle.len(),
                new_ids.len()
            ),
            Ok(Command::MarkReused { reused }) => format!("REUSED {:?}", reused.len()),
            Ok(Command::Close { .. }) => "CLOSE".to_owned(),
            Err(_) => "ERR".to_owned(),
        };

        println!(
            "CYCLES_LENGTH={:?} NMSG={:?} MSG={:?}",
            self.cycles.list.len(),
            self.recv.len(),
            msg
        );

        let mut total = 0;

        for (index, c) in self.cycles.list.iter().enumerate() {
            // let n_none = c
            //     .values()
            //     .fold(0, |acc, n| if n.is_none() { acc + 1 } else { acc });

            // println!(
            //     "CYCLE[{:?}]_LENGTH = {:?} NONE={:?}",
            //     index,
            //     c.len(),
            //     n_none
            // );

            println!("CYCLE[{:?}]_LENGTH={:?}", index, c.len(),);

            total += c.len();
        }
        println!(
            "PENDING={:?} TOTAL_IN_CYCLES={:?}",
            self.pending.len(),
            total
        );
    }

    fn start_new_cycle(
        &mut self,
        new_cycle: BTreeMap<HashId, Arc<[u8]>>,
        new_ids: ChunkedVec<HashId>,
    ) {
        GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);

        let mut hashid_without_value = Vec::with_capacity(1024);

        // let mut without_value = 0;

        for hash_id in new_ids.iter() {
            if !new_cycle.contains_key(hash_id) {
                hashid_without_value.push(*hash_id);
            }
            // new_cycle.entry(*hash_id).or_insert_with(|| {
            //     without_value += 1;
            //     None
            // });
        }

        self.send_unused(hashid_without_value);

        // let n_none = new_cycle
        //     .values()
        //     .fold(0, |acc, n| if n.is_none() { acc + 1 } else { acc });

        // println!(
        //     "GC_WORKER: START_NEW_CYCLE Got HashId without value: {:?} NEW_CYCLE_NONE={:?} NEW_CYCLE={:?} DIFF={:?}",
        //     without_value,
        //     n_none,
        //     new_cycle.len(),
        //     new_cycle.len() - n_none,
        // );

        println!("GC_WORKER: START_NEW_CYCLE NEW_CYCLE={:?}", new_cycle.len(),);

        let unused = self.cycles.roll(new_cycle);
        self.send_unused(unused);
    }

    /// Notify the main thread that the ids are free to reused
    fn send_unused(&mut self, unused: Vec<HashId>) {
        let unused_length = unused.len();
        let navailable = self.free_ids.available();

        let (to_send, pending) = if navailable < unused_length {
            unused.split_at(navailable)
        } else {
            (&unused[..], &[][..])
        };

        if let Err(e) = self.free_ids.push_slice(to_send) {
            eprintln!("GC: Fail to send free ids {:?}", e);
            self.pending.extend_from_slice(&unused);
            GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);
            return;
        }

        if !pending.is_empty() {
            self.pending.extend_from_slice(pending);
            GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);
        }
    }

    fn send_pending(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        let navailable = self.free_ids.available();
        if navailable == 0 {
            return;
        }

        let n_to_send = navailable.min(self.pending.len());
        let start = self.pending.len() - n_to_send;
        let to_send = &self.pending[start..];

        if let Err(e) = self.free_ids.push_slice(to_send) {
            eprintln!("GC: Fail to send free ids {:?}", e);
            return;
        }

        self.pending.truncate(start);
        GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);
    }

    fn mark_reused(&mut self, mut reused: Vec<HashId>) {
        GC_PENDING_HASHIDS.store(self.pending.len(), Ordering::Release);

        let mut none = 0;
        let mut total = 0;

        while let Some(hash_id) = reused.pop() {
            total += 1;

            let value = match self.cycles.move_to_last_cycle(hash_id) {
                Some(v) => v,
                None => {
                    none += 1;
                    continue;
                }
            };

            for hash_id in iter_hash_ids(&value) {
                reused.push(hash_id);
            }
        }

        println!(
            "MARK_REUSED NONE_VALUES={:?} TOTAL={:?} MOVED={:?}",
            none,
            total,
            total - none
        );

        self.send_pending();
    }
}
