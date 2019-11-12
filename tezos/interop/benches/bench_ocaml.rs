// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]

extern crate test;

use test::Bencher;

use ocaml::Str;

use tezos_interop::runtime;
use tezos_interop::runtime::OcamlResult;

fn ocaml_fn_echo(arg: String) -> OcamlResult<String> {
    runtime::spawn(move || {
        let ocaml_function = ocaml::named_value("echo").expect("function 'echo' is not registered");
        let ocaml_result: Str = ocaml_function.call::<Str>(arg.as_str().into()).unwrap().into();
        ocaml_result.as_str().to_string()
    })
}

#[bench]
fn bench_ocaml_echo(b: &mut Bencher) {
    // Run one dummy task so ocaml runtime is started. We do not want to measure
    // runtime startup time but only a time of a method call.
    futures::executor::block_on(ocaml_fn_echo("__dummy__".into())).unwrap();

    b.iter(|| futures::executor::block_on(ocaml_fn_echo("Hello world!".into())));
}