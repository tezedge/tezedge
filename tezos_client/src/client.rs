use networking::p2p::binary_message::Hexable;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi;

pub fn init_storage(storage_data_dir: String) -> (ChainId, BlockHash) {
    let (chain_id, current_block_header_hash) = ffi::init_storage(storage_data_dir)
        .expect("Ffi 'init_storage' failed! Initialization of Tezos storage failed, this storage is required, we can do nothing without that!");
    (
        hex::decode(chain_id).unwrap(),
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

pub fn apply_block(
    block_header_hash: &BlockHash,
    block_header: &BlockHeader,
    operations: &Vec<OperationsForBlocksMessage>) -> String {

    // TODO: kontrola ze mame pre vsetky validaion_pass OperationForBlock, aby sme zbytocne nevolali ocaml

    let validation_result = ffi::apply_block(
            hex::encode(block_header_hash),
            block_header.as_hex(),
            to_hex_vec(operations)
    ).expect("Ffi 'apply_block' failed! Something is wrong!");

    // TODO: what to return? validation_result? context_hash? context?
    // TODO: returning just validation_message now
    validation_result
}

fn to_hex_vec(block_operations: &Vec<OperationsForBlocksMessage>) -> Vec<Vec<String>> {
    block_operations
        .into_iter()
        .map(|bo| {
            bo.operations
                .iter()
                .map(|op| op.as_hex())
                .collect()
        })
        .collect()
}