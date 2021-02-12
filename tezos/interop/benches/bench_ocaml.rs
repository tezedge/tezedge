// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]

extern crate test;

use test::Bencher;

use ocaml_interop::{ocaml, ocaml_alloc, ocaml_call, ocaml_frame, FromOCaml, ToOCaml};

use tezos_interop::runtime;
use tezos_interop::runtime::OcamlResult;

ocaml! {
    pub fn echo(value: String) -> String;
}

fn ocaml_fn_echo(arg: String) -> OcamlResult<String> {
    runtime::spawn(move || {
        ocaml_frame!(gc, {
            let value = ocaml_alloc!(arg.to_ocaml(gc));
            let ocaml_result = ocaml_call!(echo(gc, value));
            String::from_ocaml(ocaml_result.unwrap())
        })
    })
}

#[bench]
fn bench_ocaml_echo(b: &mut Bencher) {
    // Run one dummy task so ocaml runtime is started. We do not want to measure
    // runtime startup time but only a time of a method call.
    futures::executor::block_on(ocaml_fn_echo("__dummy__".into())).unwrap();

    b.iter(|| futures::executor::block_on(ocaml_fn_echo("Hello world!".into())));
}
