use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

use crossbeam_channel::Receiver;

use crate::{kv_store::HashId, working_tree::Entry};

use tezos_spsc::Producer;

pub(crate) const PRESERVE_CYCLE_COUNT: usize = 7;

pub(crate) struct GCThread {
    pub(crate) cycles: Cycles,
    pub(crate) free_ids: Producer<HashId>,
    pub(crate) recv: Receiver<Command>,
    pub(crate) pending: Vec<HashId>,
}

pub(crate) enum Command {
    StartNewCycle {
        values_in_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
        new_ids: Vec<HashId>,
    },
    MarkReused {
        reused: Vec<HashId>,
    },
    Exit,
}

pub(crate) struct Cycles {
    list: VecDeque<BTreeMap<HashId, Option<Arc<[u8]>>>>,
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
        let len = self.list.len();
        let mut value = None;

        for store in &mut self.list.iter_mut().take(len - 1) {
            if let Some(item) = store.remove(&hash_id).flatten() {
                value = Some(item);
            };
        }

        let value = value?;
        self.list
            .back_mut()
            .unwrap()
            .insert(hash_id, Some(Arc::clone(&value)));
        return Some(value);
    }

    fn roll(&mut self, new_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>) -> Vec<HashId> {
        let unused = self.list.pop_front().unwrap();
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
        while let Ok(msg) = self.recv.recv() {
            match msg {
                Command::StartNewCycle {
                    values_in_cycle,
                    new_ids,
                } => self.start_new_cycle(values_in_cycle, new_ids),
                Command::MarkReused { reused } => self.mark_reused(reused),
                Command::Exit => {
                    return;
                }
            }
        }
    }

    fn start_new_cycle(
        &mut self,
        mut new_cycle: BTreeMap<HashId, Option<Arc<[u8]>>>,
        new_ids: Vec<HashId>,
    ) {
        for hash_id in new_ids.into_iter() {
            new_cycle.entry(hash_id).or_insert(None);
        }
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

        self.free_ids.push_slice(&to_send).unwrap();

        if !pending.is_empty() {
            self.pending.extend_from_slice(pending);
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

        self.free_ids.push_slice(&to_send).unwrap();
        self.pending.truncate(start);
    }

    fn mark_reused(&mut self, mut reused: Vec<HashId>) {
        while let Some(hash_id) = reused.pop() {
            let value = match self.cycles.move_to_last_cycle(hash_id) {
                Some(v) => v,
                None => continue,
            };

            let entry: Entry = match bincode::deserialize(&value) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("WorkingTree GC: error while decerializing entry: {:?}", err);
                    continue;
                }
            };

            match entry {
                Entry::Blob(_) => {}
                Entry::Tree(tree) => {
                    // Push every entry in this directory
                    for node in tree.values() {
                        reused.push(match node.entry_hash.get() {
                            Some(hash) => hash,
                            None => continue,
                        });
                    }
                }
                Entry::Commit(commit) => {
                    // Push the root tree for this commit
                    reused.push(commit.root_hash);
                }
            }
        }
        self.send_pending();
    }
}
