// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::{ocaml_export, OCaml, OCamlBytes, OCamlRef, OCamlRuntime, ToOCaml};
use tezos_conv::*;
use tezos_protocol_ipc_messages::{NodeMessage, ProtocolMessage};

fn encode_response<'gc>(
    cr: &'gc mut OCamlRuntime,
    response: OCamlRef<OCamlNodeMessage>,
) -> OCaml<'gc, Result<OCamlBytes, String>> {
    let rust_response: NodeMessage = response.to_rust(cr);
    let bytes =
        bincode::serialize(&rust_response).map_err(|err| format!("Serialization failure: {}", err));
    bytes.to_ocaml(cr)
}

fn decode_request<'gc>(
    cr: &'gc mut OCamlRuntime,
    bytes: OCamlRef<OCamlBytes>,
) -> OCaml<'gc, Result<OCamlProtocolMessage, String>> {
    let ocaml_bytes = cr.get(bytes);
    let rust_bytes = ocaml_bytes.as_bytes();
    let result = bincode::deserialize::<ProtocolMessage>(rust_bytes)
        .map_err(|err| format!("Deserialization failure: {}", err));

    result.to_ocaml(cr)
}

#[link(name = "tezos")]
extern "C" {
    pub fn initialize_tezedge_ipc_callbacks(
        tezedge_ipc_encode_response: unsafe extern "C" fn(isize) -> isize,
        tezedge_ipc_decode_request: unsafe extern "C" fn(isize) -> isize,
    );
}

pub fn initialize_callbacks() {
    unsafe {
        initialize_tezedge_ipc_callbacks(
            tezedge_encode_ipc_response_to_ocaml_bytes,
            tezedge_decode_ipc_message_to_ocaml,
        );
    }
}

// TODO: init function to set callbacks

ocaml_export! {
    fn tezedge_encode_ipc_response_to_ocaml_bytes(
        cr,
        response: OCamlRef<OCamlNodeMessage>
    ) -> OCaml<Result<OCamlBytes, String>> {
        encode_response(cr, response)
    }

    fn tezedge_decode_ipc_message_to_ocaml(
        cr,
        bytes: OCamlRef<OCamlBytes>
    ) -> OCaml<Result<OCamlProtocolMessage, String>>{
        decode_request(cr, bytes)
    }
}
