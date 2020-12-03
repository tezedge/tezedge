// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use enum_iterator::IntoEnumIterator;

use crypto::hash::{ContextHash, HashType, ProtocolHash};
use tezos_api::environment::{TezosEnvironment, TezosEnvironmentConfiguration, TEZOS_ENV};
use tezos_api::ffi::{PatchContext, TezosRuntimeConfiguration};
use tezos_client::client;

mod common;

#[test]
fn test_init_empty_context_for_all_enviroment_nets() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_01";

    let mut genesis_commit_hashes: Vec<ContextHash> = Vec::new();
    let mut protocol_hashes: HashSet<ProtocolHash> = HashSet::new();

    // run init storage for all nets
    let iterator = TezosEnvironment::into_enum_iter();
    let mut environment_counter = 0;
    iterator.for_each(|net| {
        environment_counter += 1;

        let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
            .get(&net)
            .expect(&format!("no tezos environment configured for: {:?}", &net));

        match client::init_protocol_context(
            common::prepare_empty_dir(&storage_data_dir),
            tezos_env.genesis.clone(),
            tezos_env.protocol_overrides.clone(),
            true,
            false,
            false,
            None,
        ) {
            Err(e) => panic!(
                "Failed to initialize storage for: {:?}, Reason: {:?}",
                net, e
            ),
            Ok(init_info) => {
                if let Some(commit_hash) = &init_info.genesis_commit_hash {
                    genesis_commit_hashes.push(commit_hash.clone());
                }
                init_info
                    .supported_protocol_hashes
                    .iter()
                    .for_each(|protocol_hash| {
                        protocol_hashes.insert(protocol_hash.clone());
                    });
            }
        }
    });

    // check result - we should have
    assert_eq!(environment_counter, genesis_commit_hashes.len());
    assert!(protocol_hashes.len() > 1);

    Ok(())
}

#[test]
fn test_init_empty_context_for_sandbox_with_patch_json() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_02";

    // run init storage for all nets
    let net = TezosEnvironment::Sandbox;
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&net)
        .expect(&format!("no tezos environment configured for: {:?}", &net));

    let patch_context = PatchContext {
        key: String::from("sandbox_parameter"),
        json: String::from(
            r#" { "genesis_pubkey": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"} "#,
        ),
    };

    match client::init_protocol_context(
        common::prepare_empty_dir(&storage_data_dir),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        Some(patch_context),
    ) {
        Err(e) => panic!(
            "Failed to initialize storage for: {:?}, Reason: {:?}",
            net, e
        ),
        Ok(init_info) => {
            if let Some(commit_hash) = &init_info.genesis_commit_hash {
                assert_eq!(
                    *commit_hash,
                    HashType::ContextHash
                        .b58check_to_hash("CoVBYdAGWBoDTkiVXJEGX6FQvDN1oGCPJu8STMvaTYdeh7N3KGTz")?
                )
            } else {
                panic!("Expected some context hash")
            }
        }
    }

    Ok(())
}

#[test]
fn test_init_empty_context_for_sandbox_without_patch_json() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_03";

    // run init storage for all nets
    let net = TezosEnvironment::Sandbox;
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&net)
        .expect(&format!("no tezos environment configured for: {:?}", &net));

    match client::init_protocol_context(
        common::prepare_empty_dir(&storage_data_dir),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        None,
    ) {
        Err(e) => panic!(
            "Failed to initialize storage for: {:?}, Reason: {:?}",
            net, e
        ),
        Ok(init_info) => {
            if let Some(commit_hash) = &init_info.genesis_commit_hash {
                assert_eq!(
                    *commit_hash,
                    HashType::ContextHash
                        .b58check_to_hash("CoVewPVcrKctWXSbrRgoGD6NmkdbDhmTFk5oi1FZpEcRT3bmKxdQ")?
                )
            } else {
                panic!("Expected some context hash")
            }
        }
    }

    Ok(())
}
