use tezos_interop::ffi;

mod common;

pub const CHAIN_ID: &str = "8eceda2f";

#[test]
fn test_init_storage() {
    // init empty storage for test
    let (chain_id, current_block_header_hash) = prepare_empty_storage("test_storage_01");
    assert_eq!(false, current_block_header_hash.is_empty());

    // has current head (genesis)
    let current_head = futures::executor::block_on(
        ffi::get_current_block_header(chain_id.to_string())
    ).unwrap();
    assert_eq!(false, current_head.is_empty());
}

#[test]
fn test_fn_get_block_header_not_found_return_none() {
    // init empty storage for test
    let _ = prepare_empty_storage("test_storage_02");

    // get unknown header
    let block_header_hash = "3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a";
    let block_header = futures::executor::block_on(
        ffi::get_block_header(block_header_hash.to_string())
    ).unwrap();

    // check not found
    assert_eq!(true, block_header.is_none());
}

/// Initializes empty dir for ocaml storage
pub fn prepare_empty_storage(dir_name: &str) -> (String, String) {
    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir(dir_name);
    let (chain_id, current_block_header_hash) = futures::executor::block_on(
        ffi::init_storage(storage_data_dir_path.to_string())
    ).unwrap();
    assert_eq!(CHAIN_ID, &chain_id);
    (chain_id, current_block_header_hash)
}