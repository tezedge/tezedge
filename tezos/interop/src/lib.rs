// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
//#![forbid(unsafe_code)]

use std::sync::Once;

use ocaml_interop::{OCamlRuntime, ToOCaml};

pub mod ipc_message_encoding;

mod tezos_ffi {
    use ocaml_interop::ocaml;

    ocaml! {
        pub fn ffi_server_loop(sock_cmd_path: String);
    }
}

pub fn start_ipc_loop(sock_cmd_path: String) {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let sock_cmd_path = sock_cmd_path.to_boxroot(rt);
        tezos_ffi::ffi_server_loop(rt, &sock_cmd_path);
    })
    .unwrap()
    // TODO: remove unwrap
}

/// Initializes the ocaml runtime and the tezos-ffi callback mechanism.
fn setup() -> OCamlRuntime {
    static INIT: Once = Once::new();
    let ocaml_runtime = OCamlRuntime::init();

    INIT.call_once(|| {
        tezos_context::ffi::initialize_callbacks();
        ipc_message_encoding::initialize_callbacks();
    });

    ocaml_runtime
}

pub fn shutdown() {
    runtime::shutdown()
}

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
