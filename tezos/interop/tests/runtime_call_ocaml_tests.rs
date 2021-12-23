// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml_interop::OCamlRuntime;
use tezos_interop::runtime::{self, OCamlBlockPanic};

#[test]
fn can_complete_future_with_return_value() -> Result<(), OCamlBlockPanic> {
    let ocaml_result = runtime::execute(|_rt: &mut OCamlRuntime| "Hello runtime!")?;
    assert_eq!("Hello runtime!", ocaml_result);
    Ok(())
}

#[test]
fn can_complete_future_with_error() {
    let res = runtime::execute(|_rt: &mut OCamlRuntime| {
        panic!("Error occurred");
    });
    assert!(res.is_err())
}
