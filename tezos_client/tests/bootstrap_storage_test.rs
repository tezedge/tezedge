use tezos_interop::ffi;

mod common;

#[test]
fn test_bootstrap_empty_storage_with_first_three_blocks() {
    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir("bootstrap_test_storage");
    let (chain_id, current_block_header_hash) = futures::executor::block_on(
        ffi::init_storage(storage_data_dir_path.to_string())
    );
    assert_eq!("TODO:chain", &chain_id);
    assert_eq!("TODO: currenet", &current_block_header_hash);

    // current head must be set (genesis)
    let current_header = futures::executor::block_on(
        ffi::get_current_block_header(chain_id.clone())
    );
    assert_eq!(false, current_header.is_empty());
    // TODO: check ze zacinam na genesis alebo header ma level 0

    // apply first block
    let ocaml_result = futures::executor::block_on(
        ffi::apply_block(
            common::test_data::BLOCK_HEADER_HASH_LEVEL_1.to_string(),
            common::test_data::BLOCK_HEADER_LEVEL_1.to_string(),
            common::test_data::block_header_level1_operations(),
        )
    );
    assert_eq!("activate PsddFKi32cMJ", &ocaml_result);

    // check current head changed to level 1
    let current_header = futures::executor::block_on(
        ffi::get_current_block_header(chain_id.clone())
    );
    // TODO: check ze ma level 1
    assert_eq!(false, current_header.is_empty());

    // apply second block
    let ocaml_result = futures::executor::block_on(
        ffi::apply_block(
            common::test_data::BLOCK_HEADER_HASH_LEVEL_2.to_string(),
            common::test_data::BLOCK_HEADER_LEVEL_2.to_string(),
            common::test_data::block_header_level2_operations(),
        )
    );
    assert_eq!("lvl 2, fit 2, prio 5, 0 ops", &ocaml_result);

    // check current head changed to level 2
    let current_header = futures::executor::block_on(
        ffi::get_current_block_header(chain_id.clone())
    );
    // TODO: check ze ma level 2
    assert_eq!(false, current_header.is_empty());

    // apply third block
    let ocaml_result = futures::executor::block_on(
        ffi::apply_block(
            common::test_data::BLOCK_HEADER_HASH_LEVEL_3.to_string(),
            common::test_data::BLOCK_HEADER_LEVEL_3.to_string(),
            common::test_data::block_header_level3_operations(),
        )
    );
    assert_eq!("lvl 3, fit 5, prio 12, 1 ops", &ocaml_result);

    // check current head changed to level 3
    let current_header = futures::executor::block_on(
        ffi::get_current_block_header(chain_id.clone())
    );
    // TODO: check ze ma level 3
    assert_eq!(false, current_header.is_empty());
}