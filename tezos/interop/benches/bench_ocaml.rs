// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bencher::{benchmark_group, benchmark_main, Bencher};

use ocaml_interop::{ocaml, OCamlRuntime, ToOCaml};

use tezos_interop::runtime::{self, OCamlCallResult};

ocaml! {
    pub fn echo(value: String) -> String;
}

fn ocaml_fn_echo(arg: String) -> OCamlCallResult<String> {
    runtime::spawn(move |rt: &mut OCamlRuntime| {
        let value = arg.to_boxroot(rt);
        echo(rt, &value).to_rust(rt)
    })
}

fn bench_ocaml_echo(b: &mut Bencher) {
    // Run one dummy task so ocaml runtime is started. We do not want to measure
    // runtime startup time but only a time of a method call.
    futures::executor::block_on(ocaml_fn_echo("__dummy__".into())).unwrap();

    b.iter(|| futures::executor::block_on(ocaml_fn_echo("Hello world!".into())));
}

benchmark_group!(benches, bench_ocaml_echo);
benchmark_main!(benches);
