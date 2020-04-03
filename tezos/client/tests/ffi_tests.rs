// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_api::environment::{self, TezosEnvironment};
use tezos_api::ffi::{OcamlStorageInitInfo, TezosRuntimeConfiguration};
use tezos_interop::ffi;

mod common;

pub const CHAIN_ID: &str = "8eceda2f";

#[test]
fn test_init_storage_and_change_configuration() {
    // change cfg
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc()
        }
    ).unwrap().unwrap();

    // init empty storage for test
    let OcamlStorageInitInfo { chain_id, genesis_block_header_hash, genesis_block_header, current_block_header_hash, .. } = prepare_empty_storage("test_storage_01");
    assert!(!current_block_header_hash.is_empty());
    assert!(!genesis_block_header.is_empty());
    assert_eq!(genesis_block_header_hash, current_block_header_hash);

    // has current head (genesis)
    let current_head = ffi::get_current_block_header(chain_id.clone()).unwrap().unwrap();
    assert!(!current_head.is_empty());

    // get header - genesis
    let block_header = ffi::get_block_header(chain_id, genesis_block_header_hash).unwrap().unwrap();

    // check header found
    assert!(block_header.is_some());
    assert_eq!(current_head, block_header.unwrap());
    assert_eq!(current_head, genesis_block_header);
}

#[test]
fn test_fn_get_block_header_not_found_return_none() {
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc()
        }
    ).unwrap().unwrap();

    // init empty storage for test
    let OcamlStorageInitInfo { chain_id, .. } = prepare_empty_storage("test_storage_02");

    // get unknown header
    let block_header_hash = hex::decode("3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a").unwrap();
    let block_header = ffi::get_block_header(chain_id, block_header_hash).unwrap().unwrap();

    // check not found
    assert!(block_header.is_none());
}

/// Initializes empty dir for ocaml storage
fn prepare_empty_storage(dir_name: &str) -> OcamlStorageInitInfo {
    let cfg = environment::TEZOS_ENV.get(&TezosEnvironment::Alphanet).expect("no tezos environment configured");

    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir(dir_name);
    let storage_init_info = ffi::init_storage(
        storage_data_dir_path.to_string(),
        &cfg.genesis,
        &cfg.protocol_overrides,
        false
    ).unwrap().unwrap();

    let expected_chain_id = hex::decode(CHAIN_ID).unwrap();
    assert_eq!(expected_chain_id, storage_init_info.chain_id);
    storage_init_info
}