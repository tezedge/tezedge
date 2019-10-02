use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use colored::*;

const GIT_REPO_URL: &str = "https://gitlab.com/simplestaking/tezos.git";
const GIT_COMMIT_HASH: &str = "6cdd4ff984f0c7bed07f6ba19c257aba2b89da48";
const GIT_REPO_DIR: &str = "lib_tezos/src";

const ARTIFACTS_DIR: &str = "lib_tezos/artifacts";
const OPAM_CMD: &str = "opam";

const REMOTE_LIB_URL: &str = "https://gitlab.com/simplestaking/tezos/uploads/f5b3bbbc4f7ff2cd111a304c8b8245c3/libtezos-ffi.so";

fn run_builder(build_chain: &str) {

    match build_chain.as_ref() {
        "local" => {
            check_prerequisites();

            // $ pushd lib_tezos && make clean all && popd
            Command::new("make")
                .args(&["clean", "all"])
                .current_dir("lib_tezos")
                .status()
                .expect("Couldn't run builder. Do you have opam and dune installed on your machine?");
        }
        "remote" => {
            // $ curl <remote_url> --output lib_tezos/artifacts/libtezos.o
            Command::new("curl")
                .args(&[REMOTE_LIB_URL, "--output", Path::new("lib_tezos").join("artifacts").join("libtezos.o").as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't retrieve compiled tezos binary.");

            // $ pushd lib_tezos/artifacts && ar qs libtezos.a libtezos.o && popd
            Command::new("ar")
                .args(&["qs", "libtezos.a", "libtezos.o"])
                .current_dir(Path::new("lib_tezos").join("artifacts").as_os_str().to_str().unwrap())
                .status()
                .expect("Couldn't run ar.");
        }
        _ => unimplemented!("cargo:warning=Invalid OCaml build chain '{}' .", build_chain)
    };
}

fn update_git_repository() {
    if !Path::new(GIT_REPO_DIR).exists() {
        Command::new("git")
            .args(&["clone", GIT_REPO_URL, GIT_REPO_DIR])
            .status()
            .expect(&format!("Couldn't clone git repository {} into {}", GIT_REPO_URL, GIT_REPO_DIR));
    }

    Command::new("git")
        .args(&["reset", "--hard", GIT_COMMIT_HASH])
        .current_dir(GIT_REPO_DIR)
        .status()
        .expect(&format!("Failed to checkout commit hash {}", GIT_COMMIT_HASH));
}

fn check_prerequisites() {
    // check if opam is installed
    let output = Command::new(OPAM_CMD).output();
    if output.is_err() || !output.unwrap().status.success() {
        println!("{}: '{}' command was not found!", "error".bright_red(), OPAM_CMD);
        println!("{}: to install opam run", "help".bright_white());
        println!("# {}", "wget https://github.com/ocaml/opam/releases/download/2.0.5/opam-2.0.5-x86_64-linux".bright_white());
        println!("# {}", "sudo cp opam-2.0.5-x86_64-linux /usr/local/bin/opam".bright_white());
        println!("# {}", "sudo chmod a+x /usr/local/bin/opam".bright_white());
        panic!();
    }

    // check if opam was initialized
    if !Path::new(&env::var("HOME").unwrap()).join(".opam").exists() {
        println!("{}: opam is not initialized", "error".bright_red());
        println!("{}: to initialize opam run", "help".bright_white());
        println!("# {}", "opam init".bright_white());
        panic!();
    }
}

fn rerun_if_ocaml_file_changes() {
    fs::read_dir(Path::new("lib_tezos"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .map(|path| (path.as_os_str().to_str().unwrap().to_string(), path.file_name().unwrap().to_str().unwrap().to_string()))
        .filter(|(_, file_name)| file_name.ends_with("ml")|| file_name.ends_with("mli"))
        .for_each(|(path, _)| println!("cargo:rerun-if-changed={}", path));
}

fn main() {
    // check we want to update git updates or just skip updates, because of development process and changes on ocaml side, which are not yet in git
    let update_git = env::var("UPDATE_GIT").unwrap_or("true".to_string()).parse::<bool>().unwrap();
    if update_git {
        update_git_repository();
    }

    // ensure lib_tezos/artifacts directory is empty
    if Path::new(ARTIFACTS_DIR).exists() {
        fs::remove_dir_all(ARTIFACTS_DIR).expect("Failed to delete artifacts directory!");
    }
    fs::create_dir_all(ARTIFACTS_DIR).expect("Failed to create artifacts directory!");

    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or("local".to_string());
    run_builder(&build_chain);

    // copy artifact files to OUT_DIR location
    let out_dir = env::var("OUT_DIR").unwrap();

    let artifacts_dir_items = fs::read_dir(ARTIFACTS_DIR).unwrap()
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<PathBuf>>();
    let artifacts_dir_items = artifacts_dir_items.iter().map(|p| p.as_path()).collect();
    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.overwrite = true;
    let bytes_copied = fs_extra::copy_items(&artifacts_dir_items, &out_dir, &copy_options).expect("Failed to copy artifacts to build output directory.");
    if bytes_copied == 0 {
        println!("cargo:warning=No files were found in the artifacts directory.");
        panic!("Failed to build tezos_interop artifacts.");
    }

    rerun_if_ocaml_file_changes();

    println!("cargo:rustc-link-search={}", &out_dir);
    println!("cargo:rustc-link-lib=dylib=tezos");
    println!("cargo:rerun-if-env-changed=OCAML_LIB");
    println!("cargo:rerun-if-env-changed=UPDATE_GIT_SUBMODULES");
}
