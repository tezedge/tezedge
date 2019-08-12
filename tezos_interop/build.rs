use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_builder(lib_dir: &PathBuf) {
    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or("docker".to_string());

    match build_chain.as_ref() {
        "local" => {
            // $ opam config exec make
            Command::new("opam")
                .args(&["config", "exec", "make"])
                .current_dir(lib_dir)
                .status()
                .expect("Couldn't run builder. Do you have opam and dune installed on your machine?");
        }
        "docker" => {
            // $ docker build -t lib_ocaml -f ../../docker/Dockerfile .
            Command::new("docker")
                .args(&["build", "-t", "lib_ocaml", "-f", "../../docker/Dockerfile", "."])
                .current_dir(&lib_dir)
                .status()
                .expect("Couldn't run docker build.");

            // $ docker run -w /home/appuser/build --name lib_ocaml lib_ocaml make
            Command::new("docker")
                .args(&["run", "-w", "/home/appuser/build", "--name", "lib_ocaml", "lib_ocaml", "make"])
                .status()
                .expect("Couldn't run build process inside of the docker container.");

            // $ docker cp lib_ocaml:/home/appuser/build/artifacts <lib_dir>/artifacts
            Command::new("docker")
                .args(&["cp", "lib_ocaml:/home/appuser/build/artifacts/.", lib_dir.join("artifacts").as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't copy files from container to host.");

            // $ docker container rm lib_ocaml
            Command::new("docker")
                .args(&["container", "rm", "lib_ocaml"])
                .status()
                .expect("Couldn't remove the docker container.");
        }
        _ => unimplemented!("cargo:warning=Invalid OCaml build chain '{}' .", build_chain)
    };
}

fn rerun_if_ocaml_file_changes(lib_dir: &PathBuf) {
    fs::read_dir(lib_dir.as_path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .map(|path| (path.as_os_str().to_str().unwrap().to_string(), path.file_name().unwrap().to_str().unwrap().to_string()))
        .filter(|(_, file_name)| file_name.ends_with("ml")|| file_name.ends_with("mli"))
        .for_each(|(path, _)| println!("cargo:rerun-if-changed={}", path));
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let lib_name = env::var("OCAML_LIB").unwrap_or("tzmock".to_string());
    let lib_dir = Path::new("lib_ocaml").join(&lib_name);
    run_builder(&lib_dir);

    // copy artifact files to OUT_DIR location
    let artifacts_dir_items = fs::read_dir(lib_dir.join("artifacts").as_path()).unwrap().filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<PathBuf>>();
    let artifacts_dir_items = artifacts_dir_items.iter().map(|p| p.as_path()).collect();
    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.overwrite = true;
    let bytes_copied = fs_extra::copy_items(&artifacts_dir_items, &out_dir, &copy_options).expect("Failed to copy artifacts to build output directory.");
    if bytes_copied == 0 {
        println!("cargo:warning=No files were copied over from artifacts directory.")
    }

    rerun_if_ocaml_file_changes(&lib_dir);

    println!("cargo:rustc-link-search={}", &out_dir);
    println!("cargo:rustc-link-lib=dylib={}", &lib_name);
    println!("cargo:rerun-if-env-changed=OCAML_LIB");
}
