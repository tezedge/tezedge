// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::{ocaml_export, OCaml, OCamlBytes, OCamlRef, OCamlRuntime, ToOCaml};
use tezos_conv::*;
use tezos_wrapper::service::{NodeMessage, ProtocolMessage};

fn encode<'gc>(
    cr: &'gc mut OCamlRuntime,
    response: OCamlRef<OCamlNodeMessage>,
) -> OCaml<'gc, OCamlBytes> {
    let rust_response: NodeMessage = response.to_rust(cr);
    let bytes = bincode::serialize(&rust_response).unwrap(); // TODO: remove unwrap
    bytes.to_ocaml(cr)
}

fn decode<'gc>(
    cr: &'gc mut OCamlRuntime,
    bytes: OCamlRef<OCamlBytes>,
) -> OCaml<'gc, Result<OCamlProtocolMessage, String>> {
    let ocaml_bytes = cr.get(bytes);
    let rust_bytes = ocaml_bytes.as_bytes();
    let result: Result<ProtocolMessage, &str> =
        bincode::deserialize(rust_bytes).map_err(|_| "Deserialization failure");

    result.to_ocaml(cr)
}

// TODO: init function to set callbacks

ocaml_export! {
    fn tezedge_encode_ipc_response_to_ocaml_bytes(
        cr,
        response: OCamlRef<OCamlNodeMessage>
    ) -> OCaml<OCamlBytes> {
        encode(cr, response)
    }

    fn tezedge_decode_ipc_message_to_ocaml(
        cr,
        bytes: OCamlRef<OCamlBytes>
    ) -> OCaml<Result<OCamlProtocolMessage, String>>{
        decode(cr, bytes)
    }

}
