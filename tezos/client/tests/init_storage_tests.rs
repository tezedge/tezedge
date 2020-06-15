// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use enum_iterator::IntoEnumIterator;

use crypto::hash::{BlockHash, ChainId, ProtocolHash};
use tezos_api::environment::{TEZOS_ENV, TezosEnvironment, TezosEnvironmentConfiguration};
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_client::client;

mod common;

#[test]
fn test_init_empty_context_for_all_enviroment_nets() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(
        TezosRuntimeConfiguration {
            debug_mode: false,
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_01";

    let mut chains: HashSet<ChainId> = HashSet::new();
    let mut genesises: HashSet<BlockHash> = HashSet::new();
    let mut protocol_hashes: HashSet<ProtocolHash> = HashSet::new();

    // run init storage for all nets
    let iterator = TezosEnvironment::into_enum_iter();
    let mut environment_counter = 0;
    iterator.for_each(|net| {
        environment_counter += 1;

        let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&net).expect(&format!("no tezos environment configured for: {:?}", &net));

        match client::init_protocol_context(
            common::prepare_empty_dir(&storage_data_dir),
            tezos_env.genesis.clone(),
            tezos_env.protocol_overrides.clone(),
            true,
            false,
            false,
        ) {
            Err(e) => panic!("Failed to initialize storage for: {:?}, Reason: {:?}", net, e),
            Ok(init_info) => {
                chains.insert(tezos_env.main_chain_id().expect("chain_id error"));
                genesises.insert(tezos_env.genesis_header_hash().expect("genesis_hash error"));

                init_info.supported_protocol_hashes.iter().for_each(|protocol_hash| {
                    protocol_hashes.insert(protocol_hash.clone());
                });
            }
        }
    });

    // check result - we should have
    assert_eq!(environment_counter, chains.len());
    assert_eq!(environment_counter, genesises.len());
    assert!(protocol_hashes.len() > 1);

    Ok(())
}
