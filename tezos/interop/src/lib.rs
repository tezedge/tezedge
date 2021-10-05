// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
//#![forbid(unsafe_code)]

use std::sync::Once;

use crypto::hash::ProtocolHash;
use ocaml_interop::{OCamlRuntime, ToOCaml};
use tezos_api::ffi::{ContextDataError, RustBytes, TezosErrorTrace};
use tezos_conv::OCamlTezosErrorTrace;

pub mod ipc_message_encoding;

type TzResult<T> = Result<T, OCamlTezosErrorTrace>;

mod tezos_ffi {
    use ocaml_interop::{ocaml, OCamlBytes, OCamlList};

    use crate::TzResult;

    ocaml! {
        pub fn ffi_server_loop(sock_cmd_path: String);
        pub fn ffi_apply_encoded_message(msg: OCamlBytes) -> Result<OCamlBytes, String>;
        pub fn decode_context_data(
            protocol_hash: OCamlBytes,
            key: OCamlList<OCamlBytes>,
            data: OCamlBytes
        ) -> TzResult<Option<OCamlBytes>>;
    }
}

pub fn decode_context_data(
    protocol_hash: ProtocolHash,
    key: Vec<String>,
    data: RustBytes,
) -> Result<Option<String>, ContextDataError> {
    let protocol_hash: RustBytes = protocol_hash.into();
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let protocol_hash = protocol_hash.to_boxroot(rt);
        let key_list = key.to_boxroot(rt);
        let data = data.to_boxroot(rt);

        let result = tezos_ffi::decode_context_data(rt, &protocol_hash, &key_list, &data);
        let result = rt.get(&result).to_result();

        match result {
            Ok(decoded_data) => Ok(decoded_data.to_rust()),
            Err(e) => Err(ContextDataError::from(e.to_rust::<TezosErrorTrace>())),
        }
    })
    .unwrap_or_else(|p| {
        Err(ContextDataError::DecodeError {
            message: p.to_string(),
        })
    })
}

pub fn start_ipc_loop(sock_cmd_path: String) {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let sock_cmd_path = sock_cmd_path.to_boxroot(rt);
        tezos_ffi::ffi_server_loop(rt, &sock_cmd_path);
    })
    .unwrap()
    // TODO: remove unwrap
}

pub fn apply_encoded_message(
    msg: tezos_protocol_ipc_messages::ProtocolMessage,
) -> Result<tezos_protocol_ipc_messages::NodeMessage, String> {
    let encoded_msg = bincode::serialize(&msg).unwrap();

    let result: Result<Vec<u8>, String> = runtime::execute(move |rt: &mut OCamlRuntime| {
        let encoded_msg = encoded_msg.to_boxroot(rt);
        let result = tezos_ffi::ffi_apply_encoded_message(rt, &encoded_msg);
        result.to_rust(rt)
    })
    .unwrap();

    result.map(|bytes| bincode::deserialize(&bytes).unwrap())
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
