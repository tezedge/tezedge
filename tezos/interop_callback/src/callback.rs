// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module provides all the FFI callback functions.

use ocaml_interop::{ocaml_export, OCaml, OCamlRef, RawOCaml};

use tezos_context::channel::*;

extern "C" {
    fn initialize_ml_context_functions(
        ml_context_record_action: unsafe extern "C" fn(
            RawOCaml,
        ) -> RawOCaml,
        ml_context_perform_action: unsafe extern "C" fn(
            RawOCaml,
        ) -> RawOCaml,
    );
}

pub fn initialize_callbacks() {
    unsafe {
        initialize_ml_context_functions(
            real_ml_context_record_action,
            real_ml_context_perform_action,
        )
    }
}

ocaml_export! {
    fn real_ml_context_record_action(cr, context_action: OCamlRef<ContextAction>) {
        let action: ContextAction = context_action.to_rust(cr);
        context_record_action(action);
        OCaml::unit()
    }

    fn real_ml_context_perform_action(cr, context_action: OCamlRef<ContextAction>) {
        let action: ContextAction = context_action.to_rust(cr);
        context_perform_action(action);
        OCaml::unit()
    }
}

fn context_record_action(action: ContextAction) {
    context_send(action).unwrap()
}

fn context_perform_action(action: ContextAction) {
    context_send(action).unwrap()
}
