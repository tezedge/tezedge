use ocaml::Str;
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
fn can_call_ocaml_fn_echo() {
    let arg = "Hello ocaml!";
    let ocaml_future = runtime::spawn(move || {
        let ocaml_function = ocaml::named_value("echo").expect("function 'echo' not registered");
        let ocaml_result: Str = ocaml_function.call::<Str>(arg.into()).unwrap().into();
        ocaml_result.as_str().to_string()
    });
    let ocaml_result = futures::executor::block_on(ocaml_future);
    assert_eq!("Hello ocaml!", &ocaml_result)
}