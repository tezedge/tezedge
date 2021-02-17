// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]

extern crate test;

use test::Bencher;

use ocaml_interop::{ocaml, ocaml_frame, to_ocaml, OCamlRuntime, ToOCaml};

use tezos_interop::runtime::{self, OCamlCallResult};

ocaml! {
    pub fn echo(value: String) -> String;
}

fn ocaml_fn_echo(arg: String) -> OCamlCallResult<String> {
    runtime::spawn(move |rt: &mut OCamlRuntime| {
        ocaml_frame!(rt, (value_root), {
            let value = to_ocaml!(rt, arg, value_root);
            echo(rt, value).to_rust()
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
