use tezos_interop::runtime;
use ocaml::Str;

#[test]
fn can_call_ocaml_fn_health_check() {
    let arg = "Hello this is dog!";
    let ocaml_future = runtime::spawn(move || {
        let ocaml_function = ocaml::named_value("health_check").expect("function 'health_check' not registered");
        let ocaml_result: Str = ocaml_function.call::<Str>(arg.into()).unwrap().into();
        ocaml_result.as_str().to_string()
    });
    let ocaml_result = futures::executor::block_on(ocaml_future);
    assert_eq!("UP - Hello this is dog!", &ocaml_result)
}