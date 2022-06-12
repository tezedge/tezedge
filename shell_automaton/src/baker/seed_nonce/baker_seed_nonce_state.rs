// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_encoding::enc::BinWriter;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::operation::Operation;

#[cfg(feature = "fuzzing")]
use tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator;

pub type SeedNonceHash = crypto::hash::NonceHash;
pub type SeedNonce = tezos_encoding::types::SizedBytes<32>;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SeedNonceRevelationOperationWithForgedBytes {
    level: Level,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<32>))]
    nonce: SeedNonce,
    forged: Vec<u8>,
}

impl SeedNonceRevelationOperationWithForgedBytes {
    pub fn new(level: i32, nonce: SeedNonce) -> Self {
        use tezos_messages::protocol::proto_005::operation::SeedNonceRevelationOperation;
        use tezos_messages::protocol::proto_012::operation::Contents;
        let op = Contents::SeedNonceRevelation(SeedNonceRevelationOperation {
            level,
            nonce: nonce.clone(),
        });
        let mut forged = vec![];
        op.bin_write(&mut forged).unwrap();
        forged.extend_from_slice(&[0; 64]);

        Self {
            level,
            nonce,
            forged,
        }
    }

    pub fn forged(&self) -> &[u8] {
        self.forged.as_ref()
    }

    pub fn as_p2p_operation(&self, branch: BlockHash) -> Operation {
        Operation::new(branch, self.forged.clone().into())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerSeedNonceState {
    Generated {
        time: u64,
        cycle: i32,
        nonce: SeedNonce,
        nonce_hash: SeedNonceHash,
    },
    /// Block with commitment has been baked.
    Committed {
        time: u64,
        cycle: i32,
        nonce: SeedNonce,
        nonce_hash: SeedNonceHash,
    },
    /// Wait for the next cycle. All the committed nonces must be revealed
    /// in next cycle.
    CycleNextWait {
        time: u64,
        /// Next cycle in which we should reveal the seed nonce.
        cycle: i32,
        nonce: SeedNonce,
        nonce_hash: SeedNonceHash,
    },
    /// Block with revelation operation hasn't been yet cemented.
    RevealPending {
        time: u64,
        cycle: i32,
        /// Blocks after which the revelation operation was injected in mempool.
        injected_in: BTreeSet<BlockHash>,
        /// Blocks in which the revelation operation was included.
        included_in: BTreeSet<BlockHash>,
        operation: SeedNonceRevelationOperationWithForgedBytes,
        nonce_hash: SeedNonceHash,
    },
    /// Block with revelation operation has been cemented.
    RevealSuccess {
        time: u64,
        cycle: i32,
        block_hash: BlockHash,
        operation: SeedNonceRevelationOperationWithForgedBytes,
        nonce_hash: SeedNonceHash,
    },
}
