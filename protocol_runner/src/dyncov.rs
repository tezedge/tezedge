// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::{ocaml_export, OCaml, OCamlRef, OCamlRuntime};

#[link(name = "tezos")]
extern "C" {
    pub fn initialize_tezedge_gcov_callbacks(
        protocol_runner_dump_gcov: unsafe extern "C" fn(isize) -> isize,
    );
}

pub fn initialize_callbacks() {
    unsafe {
        initialize_tezedge_gcov_callbacks(protocol_runner_dump_gcov);
    }
}

ocaml_export! {
    fn protocol_runner_dump_gcov(rt, _arg: OCamlRef<()>) {
        extern "C" {
            fn __gcov_dump();
            fn __gcov_reset();
        }

        unsafe {
            __gcov_dump();
            __gcov_reset();
        }

        OCaml::unit()
    }
}
