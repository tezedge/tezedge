use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;

#[test]
fn can_complete_future_with_return_value() -> Result<(), OcamlError> {
    let ocaml_result = runtime::execute(|| "Hello runtime!")?;
    Ok(assert_eq!("Hello runtime!", ocaml_result))
}

#[test]
fn can_complete_future_with_unregistered_function() {
    let res = runtime::execute(|| {
        let _ = ocaml::named_value("__non_existing_fn")
            .expect("function '__non_existing_fn' is not registered");
    });
    assert!(res.is_err())
}

#[test]
fn can_complete_future_with_error() {
    let res = runtime::execute(|| {
        panic!("Error occurred");
    });
    assert!(res.is_err())
}