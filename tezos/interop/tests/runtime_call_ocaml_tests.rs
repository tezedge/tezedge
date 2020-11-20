use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;

#[test]
fn can_complete_future_with_return_value() -> Result<(), OcamlError> {
    let ocaml_result = runtime::execute(|| "Hello runtime!")?;
    Ok(assert_eq!("Hello runtime!", ocaml_result))
}

#[test]
fn can_complete_future_with_error() {
    let res = runtime::execute(|| {
        panic!("Error occurred");
    });
    assert!(res.is_err())
}
