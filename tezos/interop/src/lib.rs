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
/// use tezos_interop::runtime::OcamlResult;
/// use tezos_interop::runtime;
/// use ocaml_interop::{ocaml, ocaml_frame, ocaml_alloc, ocaml_call, ToOCaml, FromOCaml};
///
/// ocaml! {
///     pub fn echo(value: String) -> String;
/// }
///
/// fn ocaml_fn_echo(arg: String) -> OcamlResult<String> {
///     runtime::spawn(move || {
///         ocaml_frame!(gc, {
///             let value = ocaml_alloc!(arg.to_ocaml(gc));
///             let ocaml_result = ocaml_call!(echo(gc, value));
///             String::from_ocaml(ocaml_result.unwrap())
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
