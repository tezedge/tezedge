use tezos_interop::ffi;

#[test]
fn can_call_ocaml_fn_get_block_header_not_found() {
    // "BLwKksYwrxt39exDei7yi47h7aMcVY2kZMZhTwEEoSUwToQUiDV"
    let block_header_hash = "60ab6d8d2a6b1c7a391f00aa6c1fc887eb53797214616fd2ce1b9342ad4965a4";

    let ocaml_result = ffi::get_block_header(block_header_hash.to_string());
    let ocaml_result = futures::executor::block_on(ocaml_result);

    assert_eq!(true, ocaml_result.is_none());
}

