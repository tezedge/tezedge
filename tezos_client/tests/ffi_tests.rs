use tezos_interop::ffi;

#[test]
fn can_call_ocaml_fn_get_block_header_not_found() {
    let block_header_hash = "3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a";

    let ocaml_result = ffi::get_block_header(block_header_hash.to_string());
    let ocaml_result = futures::executor::block_on(ocaml_result);

    assert_eq!(true, ocaml_result.is_none());
}

#[test]
fn test_call_ocaml_fn_get_current_block_header() {
    // TODO: init storage a test current headu

    let chain_id = "TODO:chain_id:NetXgtSLGNJvNye";

    let current_head = ffi::get_current_block_header(chain_id.to_string());
    let current_head = futures::executor::block_on(current_head);

    assert_eq!(false, current_head.is_empty());
}