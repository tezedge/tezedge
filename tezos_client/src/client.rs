use networking::p2p::binary_message::Hexable;
use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, ChainId};
use tezos_interop::ffi;

// TODO: return Result a potomt zmeni unwrap za ? ???
// TODO: potrebujeme totok async?
// TODO: realne napojit na light_node
// TODO: zmenit genesis default chain a napojit na storage

pub fn init_storage(storage_data_dir: String) -> (ChainId, BlockHash) {
    let (chain_id, current_block_header_hash) = futures::executor::block_on(
        ffi::init_storage(storage_data_dir)
    );
    (
        hex::decode(chain_id).unwrap(),
        hex::decode(current_block_header_hash).unwrap(),
    )
}

pub fn get_current_block_header(chain_id: &ChainId) -> BlockHeader {
    let current_block_header = futures::executor::block_on(
        ffi::get_current_block_header(hex::encode(chain_id))
    );
    BlockHeader::from_hex(current_block_header)
}

pub fn get_block_header(block_header_hash: &BlockHash) -> Option<BlockHeader> {
    let block_header = futures::executor::block_on(
        ffi::get_block_header(hex::encode(block_header_hash))
    );
    block_header.map(|bh| BlockHeader::from_hex(bh))
}

pub fn apply_block(
    block_header_hash: &BlockHash,
    block_header: &BlockHeader,
    operations: &Vec<OperationsForBlocksMessage>) -> String {

    // TODO: kontrola ze mame pre vsetky validaion_pass OperationForBlock, aby sme zbytocne nevolali ocaml

    let validation_result = futures::executor::block_on(
        ffi::apply_block(
            hex::encode(block_header_hash),
            block_header.as_hex(),
            to_hex_vec(operations),
        )
    );

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