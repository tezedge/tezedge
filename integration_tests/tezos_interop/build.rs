use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dune_dir = "_build/default";
    let lib_ocaml_dir = "lib_ocaml";

    Command::new("dune")
        .args(&["build", "tzmock.so"])
        .current_dir(lib_ocaml_dir)
        .status()
        .expect("Couldn't run builder. Do you have dune installed on your machine?");
    Command::new("cp")
        .args(&[
            &format!("{}/{}/tzmock.so", lib_ocaml_dir, dune_dir),
            &format!("{}/libtzmock.o", out_dir),
        ])
        .status()
        .expect("File copy failed.");
    Command::new("ar")
        .args(&[
            "qs",
            &format!("{}/libtzmock.a", out_dir),
            &format!("{}/libtzmock.o", out_dir),
        ])
        .status()
        .expect("ar gave an error");

    println!("cargo:rustc-link-search={}", out_dir);
    println!("cargo:rustc-link-lib=dylib=tzmock")
}
