use failure::Fail;

use networking::p2p::binary_message::Hexable;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi;

#[derive(Debug, Fail)]
pub enum ApplyBlockError {
    #[fail(display = "incomplete operations")]
    IncompleteOperations,
    #[fail(display = "failed to apply block")]
    FailedToApplyBlock,
}

//        TODO: priznak do storage, ze mame block zapisany v tezos-ocaml-storage
//        TODO: spravit benches pre bootstrap test a zoptimalizovat ocaml s logovanim a bez logovania
//        TODO: jira - od coho zavisi konfiguracia alphanet vs mainnet vs zeronet: storage a genesis?
//                   - Return supported network protocol version
//        TODO: jira - Tezos_validation.Block_validation.apply
//        TODO: jira - podpora pre test_chain
//        TODO: jira - overit proof_of_work_stamp pri bootstrape
//        TODO: jira - pre generovanie identity

/// Initializes storage for Tezos ocaml storage in chosen directory
pub fn init_storage(storage_data_dir: String) -> (ChainId, BlockHash, BlockHash) {
    let (chain_id, genesis_block_header_hash, current_block_header_hash) = ffi::init_storage(storage_data_dir)
        .expect("Ffi 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that!");
    (
        hex::decode(chain_id).unwrap(),
        hex::decode(genesis_block_header_hash).unwrap(),
        hex::decode(current_block_header_hash).unwrap(),
    )
}

pub fn get_current_block_header(chain_id: &ChainId) -> BlockHeader {
    let current_block_header = ffi::get_current_block_header(hex::encode(chain_id))
        .expect("Ffi 'get_current_block_header' failed! Current head must be set all the time, if not, something is wrong!");
    BlockHeader::from_hex(current_block_header)
}

pub fn get_block_header(block_header_hash: &BlockHash) -> Option<BlockHeader> {
    let block_header = ffi::get_block_header(hex::encode(block_header_hash))
        .expect("Ffi 'get_block_header' failed! Something is wrong!");
    block_header.map(|bh| BlockHeader::from_hex(bh))
}

/// Applies new block to Tezos ocaml storage, means:
/// - block and operations are decoded by the protocol
/// - block and operations data are correctly stored in Tezos chain/storage
/// - new current head is evaluated
/// - returns validation_result.message
pub fn apply_block(
    block_header_hash: &BlockHash,
    block_header: &BlockHeader,
    operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<String, ApplyBlockError> {

    // TODO: ApplyBlockError::IncompleteOperations check for completness validaion_pass OperationForBlock

    let validation_result = ffi::apply_block(
        hex::encode(block_header_hash),
        block_header.as_hex(),
        to_hex_vec(operations),
    );

    match validation_result {
        Ok(validation_result) => {
            // TODO: what to return? validation_result? context_hash? context?
            // TODO: returning just validation_message now
            Ok(validation_result)
        },
        Err(_) => Err(ApplyBlockError::FailedToApplyBlock)
    }
}

fn to_hex_vec(block_operations: &Vec<Option<OperationsForBlocksMessage>>) -> Vec<Option<Vec<String>>> {
    block_operations
        .into_iter()
        .map(|bo| {
            if let Some(bo_ops) = bo {
                Some(
                    bo_ops.operations
                        .iter()
                        .map(|op| op.as_hex())
                        .collect()
                )
            } else {
                None
            }
        })
        .collect()
}