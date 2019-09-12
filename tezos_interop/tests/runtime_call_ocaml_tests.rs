use tezos_interop::runtime;

#[test]
fn can_complete_future_with_return_value() {
    let ocaml_future = runtime::spawn(|| {
        "Hello runtime!"
    });
    let ocaml_result = futures::executor::block_on(ocaml_future);
    assert_eq!("Hello runtime!", ocaml_result)
}

#[test]
#[should_panic]
fn can_complete_future_with_unregistered_function() {
    futures::executor::block_on(
        runtime::spawn(|| {
            let _ = ocaml::named_value("this_function_is_surelly_not_registered_in_ocaml_runtime")
                .expect("function 'this_function_is_surelly_not_registered_in_ocaml_runtime' is not registered");
        })
    );
}

#[test]
#[should_panic]
fn can_complete_future_with_error() {
    futures::executor::block_on(
        runtime::spawn(|| {
            panic!("Error occurred");
        })
    );
}