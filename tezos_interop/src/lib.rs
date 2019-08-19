#![feature(async_await, fn_traits)]

/// This modules will allow you to call OCaml code:
///
/// ```ocaml
/// let echo = fun value: string -> value
/// let _ = Callback.register "echo" echo
/// ```
///
/// It can be then easily awaited in rust:
///
/// ```rust,edition2018
/// #![feature(async_await)]
/// use tezos_interop::runtime::OcamlResult;
/// use tezos_interop::runtime;
/// use ocaml::{Str};
///
/// fn ocaml_fn_echo(arg: String) -> OcamlResult<String> {
///     runtime::spawn(move || {
///         let ocaml_function = ocaml::named_value("echo").expect("function 'echo' is not registered");
///         let ocaml_result: Str = ocaml_function.call::<Str>(arg.as_str().into()).unwrap().into();
///         ocaml_result.as_str().to_string()
///     })
/// }
///
/// async fn call_ocaml_hello_world() -> String {
///     ocaml_fn_echo("Hello world!".into()).await
/// }
///
/// let result = futures::executor::block_on(call_ocaml_hello_world());
/// assert_eq!("Hello world!", &result);
/// ```
pub mod runtime;
