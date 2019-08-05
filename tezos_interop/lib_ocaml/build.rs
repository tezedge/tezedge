use std::process::Command;
use std::env;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dune_dir = "_build/default";

    let ocaml_dirs = vec!["tzmock"];

    for lib_name in ocaml_dirs.into_iter() {
        Command::new("dune")
            .args(&["build", &format!("{}.so", lib_name)])
            .current_dir(lib_name)
            .status()
            .expect("Couldn't run builder. Do you have dune installed on your machine?");
        Command::new("cp")
            .args(&[
                &format!("{}/{}/{}.so", lib_name, lib_name, dune_dir),
                &format!("{}/lib{}.o", out_dir, lib_name),
            ])
            .status()
            .expect("File copy failed.");
        Command::new("ar")
            .args(&[
                "qs",
                &format!("{}/lib{}.a", out_dir, lib_name),
                &format!("{}/lib{}.o", out_dir, lib_name),
            ])
            .status()
            .expect("ar gave an error");
    }



    println!("cargo:rustc-link-search={}", out_dir);
}
