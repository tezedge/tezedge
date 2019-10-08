use std::collections::HashSet;

use tezos_client::client;
use tezos_client::client::TezosRuntimeConfiguration;
use tezos_client::environment::{TezosEnvironment, TezosEnvironmentVariants};
use tezos_encoding::hash::{BlockHash, ChainId};

mod common;

#[test]
fn test_init_empty_storage_for_all_enviroment_nets() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration { log_enabled: common::is_ocaml_log_enabled() }).unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_01";

    let mut chains: HashSet<ChainId> = HashSet::new();
    let mut genesises: HashSet<BlockHash> = HashSet::new();
    let mut current_heads: HashSet<BlockHash> = HashSet::new();

    // run init storage for all nets
    let iterator: TezosEnvironmentVariants = TezosEnvironment::iter_variants();
    iterator.for_each(|net| {
        match client::init_storage(
            common::prepare_empty_dir(&storage_data_dir),
            net,
        ) {
            Err(e) => panic!("Failed to initialize storage for: {:?}, Reason: {:?}", net, e),
            Ok(init_info) => {
                chains.insert(init_info.chain_id);
                genesises.insert(init_info.genesis_block_header_hash);
                current_heads.insert(init_info.current_block_header_hash);
            }
        }
    });

    // check result - we should have
    assert_eq!(4, chains.len());
    assert_eq!(4, genesises.len());
    assert_eq!(4, current_heads.len());

    Ok(())
}