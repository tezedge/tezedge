// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

pub mod ffi;
pub mod ipc_message_encoding;

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
/// use ocaml_interop::{ocaml, ToOCaml, FromOCaml, OCamlRuntime};
///
/// ocaml! {
///     pub fn echo(value: String) -> String;
/// }
///
/// fn ocaml_fn_echo(arg: String) -> OCamlCallResult<String> {
///     runtime::spawn(move |rt: &mut OCamlRuntime| {
///         let value = arg.to_boxroot(rt);
///         let ocaml_result = echo(rt, &value);
///         ocaml_result.to_rust(rt)
///     })
/// }
///
/// let result = futures::executor::block_on(
///     ocaml_fn_echo("Hello world!".into())
/// ).unwrap();
/// assert_eq!("Hello world!", &result);
/// ```
pub mod runtime;

pub fn force_libtezos_linking() {
    tezos_sys::force_libtezos_linking();
}
