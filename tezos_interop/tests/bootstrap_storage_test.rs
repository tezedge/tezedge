use tezos_interop::ffi;
mod common;

#[test]
fn test_bootstrap_empty_storage_with_first_three_blocks() {

    // apply first block
    let ocaml_result = ffi::apply_block(
        common::test_data::BLOCK_HEADER_HASH_LEVEL_1.to_string(),
        common::test_data::BLOCK_HEADER_LEVEL_1.to_string(),
        common::test_data::block_header_level1_operations()
    );
    let ocaml_result = futures::executor::block_on(ocaml_result);
    assert_eq!("activate PsddFKi32cMJ", &ocaml_result);

    // apply second block
    let ocaml_result = ffi::apply_block(
        common::test_data::BLOCK_HEADER_HASH_LEVEL_2.to_string(),
        common::test_data::BLOCK_HEADER_LEVEL_2.to_string(),
        common::test_data::block_header_level2_operations()
    );
    let ocaml_result = futures::executor::block_on(ocaml_result);
    assert_eq!("lvl 2, fit 2, prio 5, 0 ops", &ocaml_result);

    // apply third block
    let ocaml_result = ffi::apply_block(
        common::test_data::BLOCK_HEADER_HASH_LEVEL_3.to_string(),
        common::test_data::BLOCK_HEADER_LEVEL_3.to_string(),
        common::test_data::block_header_level3_operations()
    );
    let ocaml_result = futures::executor::block_on(ocaml_result);
    assert_eq!("lvl 3, fit 5, prio 12, 1 ops", &ocaml_result);
}