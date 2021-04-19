// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides all the FFI callback functions.

use ocaml_interop::{ocaml_export, OCaml, OCamlRef};

use tezos_context::channel::{context_send, ContextAction};
use tezos_sys::initialize_ml_context_functions;

pub fn initialize_callbacks() {
    unsafe { initialize_ml_context_functions(real_ml_context_send_action) }
}

ocaml_export! {
    fn real_ml_context_send_action(
        cr,
        context_action: OCamlRef<ContextAction>,
    ) {
        let action: ContextAction = context_action.to_rust(cr);
        context_send_action(action);
        OCaml::unit()
    }
}

fn context_send_action(action: ContextAction) {
    context_send(action).unwrap()
}
