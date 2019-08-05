use std::env;
use std::path::Path;
use std::process::Command;

fn build_ocaml_libs() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dune_dir = Path::new("_build").join("default");

    let ocaml_libs = vec!["tzmock"];
    for lib_name in ocaml_libs.into_iter() {
        let lib_dir = Path::new("lib_ocaml").join(lib_name);
        let lib_so = format!("{}.so", lib_name);
        let lib_o = format!("lib{}.o", lib_name);
        let lib_a = format!("lib{}.a", lib_name);

        Command::new("dune")
            .args(&["build", &lib_so])
            .current_dir(&lib_dir)
            .status()
            .expect("Couldn't run builder. Do you have dune installed on your machine?");

        Command::new("cp")
            .args(&[
                Path::new(&lib_dir).join(&dune_dir).join(&lib_so).to_str().unwrap(),
                Path::new(&out_dir).join(&lib_o).to_str().unwrap(),
            ])
            .status()
            .expect("File copy failed.");

        Command::new("ar")
            .args(&[
                "qs",
                Path::new(&out_dir).join(&lib_a).to_str().unwrap(),
                Path::new(&out_dir).join(&lib_o).to_str().unwrap(),
            ])
            .status()
            .expect("ar gave an error");
    }

    println!("cargo:rustc-link-search={}", out_dir);
}

fn link_ocaml_lib() {
    let tezos_use_mock = env::var("TEZOS_USE_MOCK").unwrap();

    let lib_name = match tezos_use_mock.as_ref() {
        "1" | "yes" => "tzmock",
        _ => unimplemented!("Invalid profile: {}", tezos_use_mock)
    };

    println!("cargo:rustc-link-lib=dylib={}", lib_name)
}

fn main() {
    build_ocaml_libs();
    link_ocaml_lib();
}
