#![feature(async_await, fn_traits)]
pub mod runtime;

/// This modules will allow you to call OCaml code:
///
/// ```ocaml
/// let echo = fun value: string -> value
/// let _ = Callback.register "echo" echo
/// ```
///
/// It can be then easily awaited in rust:
///
/// ```rust
/// #![feature(async_await)]
/// use ocaml::{Value, Str, named_value};
/// use crate::runtime;
///
/// async fn ocaml_fn_echo(arg: String) -> String {
///     runtime::spawn(move || {
///         let ocaml_function = named_value("echo").expect("function 'echo' is not registered");
///         let ocaml_result: Str = ocaml_function.call::<Str>(arg.as_str().into()).unwrap().into();
///         ocaml_result.as_str().to_string()
///     }).await
/// }
///
/// async fn call_ocaml() {
///     ocaml_fn_echo("Hello world!".into()).await;
/// }
/// ```



#[cfg(test)]
mod tests {
    use ocaml::{Value, Str};
    use crate::runtime;

    #[test]
    fn can_complete_future_with_return_value() {
        let ocaml_future = runtime::spawn(|| {
            "Hello runtime!"
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);
        assert_eq!("Hello runtime!", ocaml_result)
    }

    #[test]
    fn can_call_ocaml_fn_twice() {
        let arg = 15486;
        let ocaml_future = runtime::spawn(move || {
            let ocaml_function = ocaml::named_value("twice").expect("function not registered");
            ocaml_function.call(Value::i32(arg)).unwrap().i32_val()
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);

        assert_eq!(arg * 2, ocaml_result)
    }

    #[test]
    fn can_call_ocaml_fn_echo() {
        let arg = "Hello this is dog!";
        let ocaml_future = runtime::spawn(move || {
            let ocaml_function = ocaml::named_value("echo").expect("function not registered");
            let ocaml_result: Str = ocaml_function.call::<Str>(arg.into()).unwrap().into();
            ocaml_result.as_str().to_string()
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);

        assert_eq!(arg, &ocaml_result)
    }

    #[test]
    fn can_call_ocaml_fn_encode_operation_success() {
        let expected_value = r#"{ "contents":    [ { "kind": "transaction",        "source": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx", "fee": "1272",        "counter": "1", "gas_limit": "10200", "storage_limit": "0",        "amount": "42000000",        "destination": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN" } ],  "signature":    "sighEKUX4TVUc7hHHTb9GX1VJSYbDteH5TAXTA7bnhJo81X7mC99hnroqR4QoBkivq7xRr8ZeGC5oXsoBQao6ha9RHAk3i4c" }"#;
        let input_value = "02f2b231cb7e4a6a264ca85e3e69ef4bec6cfed8749a5ce605f43b2ca4b4d93308000002298c03ed7d454a101eb7022bc95f7e5f41ac78f80901d84f0080bd83140000e7670f32038107a59a2b9cfefae36ea21f5aa63c00931399163d0698672e8532aa15ecc679dccd6844634099e02d224cc61f6bce236819bc855e09c222f2d4056ea608c0de9d5cd633652b29cf3e1c80c6c1e0340c";
        let ocaml_future = runtime::spawn(move || {
            let ocaml_function = ocaml::named_value("encode_operation").expect("function not registered");
            let ocaml_value: Str = ocaml_function.call_exn::<Str>(input_value.into()).unwrap().into();
            ocaml_value.as_str().to_string()
        });
        let ocaml_result = futures::executor::block_on(ocaml_future).replace("\n", "");

        assert_eq!(expected_value, &ocaml_result)
    }

    #[test]
    fn can_call_ocaml_fn_encode_operation_error() {
        let input_value = "02f2b2317e46a264ca85";
        let ocaml_future = runtime::spawn(move || {
            let ocaml_function = ocaml::named_value("encode_operation").expect("function not registered");
            match ocaml_function.call_exn::<Str>(input_value.into()) {
                Ok(ref value) => Ok(Str::from(value.clone()).as_str().to_string()),
                Err(err) => Err(err)
            }
        });
        let ocaml_result = futures::executor::block_on(ocaml_future);
        assert!(ocaml_result.is_err())
    }
}
