#![feature(async_await, fn_traits)]
pub mod runtime;

#[cfg(test)]
mod tests {
    use ocaml::Value;

    use crate::runtime;

    #[test]
    fn can_complete_future_with_return_value() {
        let ocaml_future = runtime::run(|| {
            "Hello runtime!"
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);
        assert_eq!("Hello runtime!", ocaml_result)
    }

    #[test]
    fn can_call_ocaml_fn_twice() {
        let arg = 15486;
        let ocaml_future = runtime::run(move || {
            let f = ocaml::named_value("twice").expect("function not registered");
            f.call(Value::int32(arg)).unwrap().int32_val()
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);
        assert_eq!(arg * 2, ocaml_result)
    }
}
