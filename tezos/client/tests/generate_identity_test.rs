// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_interop::ffi;

pub const CHAIN_ID: &str = "8eceda2f";

#[test]
fn test_fn_generate_identity() {
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            debug_mode: false,
            log_enabled: is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: no_of_ffi_calls_treshold_for_gc()
        }
    ).unwrap().unwrap();

    // generate
    let identity = ffi::generate_identity(16f64).unwrap().unwrap();

    // check
    assert!(!identity.peer_id.is_empty());
    assert!(!identity.public_key.is_empty());
    assert!(!identity.secret_key.is_empty());
    assert!(!identity.proof_of_work_stamp.is_empty());
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("2000".to_string())
        .parse::<i32>().unwrap()
}
