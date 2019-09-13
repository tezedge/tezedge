use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;

#[test]
fn can_complete_future_with_return_value() -> Result<(), OcamlError> {
    let ocaml_future = runtime::spawn(|| {
        "Hello runtime!"
    });
    let ocaml_result = futures::executor::block_on(ocaml_future)?;
    Ok(assert_eq!("Hello runtime!", ocaml_result))
}

#[test]
fn can_complete_future_with_unregistered_function() {
    let res = futures::executor::block_on(
        runtime::spawn(|| {
            let _ = ocaml::named_value("__non_existing_fn")
                .expect("function '__non_existing_fn' is not registered");
        })
    );
    assert!(res.is_err())
}

#[test]
fn can_complete_future_with_error() {
    let res = futures::executor::block_on(
        runtime::spawn(|| {
            panic!("Error occurred");
        })
    );
    assert!(res.is_err())
}