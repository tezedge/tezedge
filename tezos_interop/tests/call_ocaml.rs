use tezos_interop::runtime;

#[test]
fn can_complete_future_with_return_value() {
    let ocaml_future = runtime::spawn(|| {
        "Hello runtime!"
    });
    let ocaml_result = futures::executor::block_on(ocaml_future);
    assert_eq!("Hello runtime!", ocaml_result)
}

