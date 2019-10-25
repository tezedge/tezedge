use tezos_encoding::hash::BlockHash;

#[derive(Debug, Clone)]
pub struct CurrentHead {
    level: i32,
    block_hash: BlockHash,
}

impl CurrentHead {
    pub fn new(level: i32, block_hash: BlockHash) -> Self {
        Self {
            level,
            block_hash,
        }
    }

    pub fn hash(&self) -> BlockHash {
        self.block_hash.clone()
    }

    pub fn level(&self) -> i32 {
        self.level
    }
}