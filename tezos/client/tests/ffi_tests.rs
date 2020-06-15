// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::HashType;
use tezos_api::environment::{self, TezosEnvironment};
use tezos_api::ffi::{InitProtocolContextResult, TezosRuntimeConfiguration};
use tezos_interop::ffi;

mod common;

#[test]
fn test_init_protocol_context() {
    // change cfg
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            debug_mode: false,
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap().unwrap();

    let context_hash_encoding = HashType::ContextHash;
    let storage_dir = "test_storage_01";
    let tezos_env = TezosEnvironment::Carthagenet;

    // init empty storage for test WITH commit genesis
    let InitProtocolContextResult {
        genesis_commit_hash,
        supported_protocol_hashes
    } = prepare_protocol_context(storage_dir, &tezos_env, true);

    // check
    assert!(!supported_protocol_hashes.is_empty());
    assert!(genesis_commit_hash.is_some());
    let genesis_commit_hash = genesis_commit_hash.unwrap();
    assert_eq!(
        context_hash_encoding.bytes_to_string(&genesis_commit_hash).as_str(),
        "CoWZVRSM6DdNUpn3mamy7e8rUSxQVWkQCQfJBg7DrTVXUjzGZGCa",
    );

    // init the same storage without commit
    let InitProtocolContextResult {
        genesis_commit_hash,
        supported_protocol_hashes
    } = prepare_protocol_context(storage_dir, &tezos_env, false);

    // check
    assert!(!supported_protocol_hashes.is_empty());
    assert!(genesis_commit_hash.is_none());
}

/// Initializes empty dir for ocaml storage
fn prepare_protocol_context(dir_name: &str, tezos_env: &TezosEnvironment, commit_genesis: bool) -> InitProtocolContextResult {
    let cfg = environment::TEZOS_ENV.get(tezos_env).expect("no tezos environment configured");

    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir(dir_name);
    let storage_init_info = ffi::init_protocol_context(
        storage_data_dir_path.to_string(),
        cfg.genesis.clone(),
        cfg.protocol_overrides.clone(),
        commit_genesis,
        false,
        false,
    ).unwrap().unwrap();

    storage_init_info
}
