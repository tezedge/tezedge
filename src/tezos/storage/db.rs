use std::collections::HashMap;

/// (Demo) Structure for represnting in-memory db for - just for demo purposes.
pub struct Db {
    branches: HashMap<String, Vec<u8>>
}

impl Db {
    pub fn new() -> Self {
        Db {
            branches: HashMap::new()
        }
    }

    pub fn store_branch(&mut self, id: String, branch_as_bytes: Vec<u8>) {
        self.branches.insert(id, branch_as_bytes);
    }

    pub fn get_branches(&self) -> Vec<Vec<u8>> {
        let mut branches = vec![];
        for (_, branch) in self.branches.iter() {
            branches.push(branch.clone());
        }
        branches
    }
}