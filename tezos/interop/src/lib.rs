#![feature(fn_traits)]

/// This modules will allow you to call OCaml code:
///
/// ```ocaml
/// let echo = fun value: string -> value
/// let _ = Callback.register "echo" echo
/// ```
///
/// It can be then easily awaited in rust:
///
/// ```
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
/// let result = futures::executor::block_on(
///     ocaml_fn_echo("Hello world!".into())
/// ).unwrap();
/// assert_eq!("Hello world!", &result);
/// ```
pub mod runtime;
pub mod ffi;
pub mod callback;
