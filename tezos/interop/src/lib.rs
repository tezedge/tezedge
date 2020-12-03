// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

pub mod ffi;
/// This modules will allow you to call OCaml code:
///
/// ```ocaml
/// let echo = fun value: string -> value
/// let _ = Callback.register "echo" echo
/// ```
///
/// It can be then easily awaited in rust:
///
/// ```rust, no_run
/// use tezos_interop::runtime::OCamlCallResult;
/// use tezos_interop::runtime;
/// use ocaml_interop::{ocaml, ocaml_frame, to_ocaml, ToOCaml, FromOCaml, OCamlRuntime};
///
/// ocaml! {
///     pub fn echo(value: String) -> String;
/// }
///
/// fn ocaml_fn_echo(arg: String) -> OCamlCallResult<String> {
///     runtime::spawn(move |rt: &mut OCamlRuntime| {
///         ocaml_frame!(rt, (root), {
///             let value = to_ocaml!(rt, arg, root);
///             let ocaml_result = echo(rt, value);
///             String::from_ocaml(ocaml_result)
///         })
///     })
/// }
///
/// let result = futures::executor::block_on(
///     ocaml_fn_echo("Hello world!".into())
/// ).unwrap();
/// assert_eq!("Hello world!", &result);
/// ```
pub mod runtime;
