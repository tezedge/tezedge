// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides all the FFI callback functions.

use ocaml_interop::{ocaml_export, OCaml, RawOCaml, ToRust};

use tezos_context::channel::{context_send, ContextAction, ContextActionMessage};

extern "C" {
    fn initialize_ml_context_functions(
        ml_context_send_action: unsafe extern "C" fn(RawOCaml, RawOCaml, RawOCaml) -> RawOCaml,
    );
}

pub fn initialize_callbacks() {
    unsafe { initialize_ml_context_functions(real_ml_context_send_action) }
}

ocaml_export! {
    fn real_ml_context_send_action(
        cr,
        context_action: OCaml<ContextAction>,
        record: OCaml<bool>,
        perform: OCaml<bool>,
    ) {
        let action: ContextAction = context_action.to_rust();
        let record = record.to_rust();
        let perform = perform.to_rust();
        context_send_action(action, record, perform);
        OCaml::unit()
    }

}

fn context_send_action(action: ContextAction, record: bool, perform: bool) {
    context_send(ContextActionMessage {
        action,
        perform,
        record,
    })
    .unwrap()
}
