use std::collections::HashMap;

use crate::tezos::p2p::peer::PeerId;


pub type BranchId = Vec<u8>;

/// Structure for representing in-memory db for - just for demo purposes.
pub struct Db {
    branches: HashMap<PeerId, BranchId>
}

impl Db {
    pub fn new() -> Self {
        Db {
            branches: HashMap::new()
        }
    }

    pub fn store_branch(&mut self, id: PeerId, branch_as_bytes: BranchId) {
        self.branches.insert(id, branch_as_bytes);
    }

    pub fn get_branches(&self) -> Vec<BranchId> {
        let mut branches = vec![];
        for (_, branch) in self.branches.iter() {
            branches.push(branch.clone());
        }
        branches
    }
}