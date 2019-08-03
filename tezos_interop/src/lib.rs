#![feature(async_await, fn_traits)]
pub mod runtime;


#[cfg(test)]
mod tests {
//    use ocaml::caml;
    use super::*;

    #[test]
    fn can_complete_future() {
        let ocaml_future = runtime::run(|| {
//            caml!(ml_testing_callback(a, b) {
//                let f = ocaml::named_value("print_testing").expect("print_testing not registered");
//                f.call_n(&[a, b]).unwrap();
//                return ocaml::Value::unit();
//            });
            "Hello runtime!"
        } );
        let ocaml_result = futures::executor::block_on(ocaml_future);
        assert_eq!("Hello runtime!", ocaml_result)
    }
}