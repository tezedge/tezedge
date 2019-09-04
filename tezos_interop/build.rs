use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const REMOTE_LIB_URL: &str = "https://gitlab.com/simplestaking/tezos/uploads/f5b3bbbc4f7ff2cd111a304c8b8245c3/libtezos-ffi.so";

fn run_builder(build_chain: &str) {

    match build_chain.as_ref() {
        "local" => {
            // $ pushd lib_tezos && make clean all && popd
            Command::new("make")
                .args(&["clean", "all"])
                .current_dir("lib_tezos")
                .status()
                .expect("Couldn't run builder. Do you have opam and dune installed on your machine?");
        }
        "docker" => {
            // $ pushd lib_tezos && docker build -t lib_tezos -f ../docker/Dockerfile . && popd
            Command::new("docker")
                .args(&["build", "-t", "lib_tezos", "-f", "../docker/Dockerfile", "."])
                .current_dir("lib_tezos")
                .status()
                .expect("Couldn't run docker build.");

            // $ docker run -w /home/appuser/build --name lib_tezos lib_tezos make
            Command::new("docker")
                .args(&["run", "-w", "/home/appuser/build", "--name", "lib_tezos", "lib_tezos", "make"])
                .status()
                .expect("Couldn't run build process inside of the docker container.");

            // $ docker cp lib_tezos:/home/appuser/build/artifacts/. lib_tezos/artifacts
            Command::new("docker")
                .args(&["cp", "lib_tezos:/home/appuser/build/artifacts/.", Path::new("lib_tezos").join("artifacts").as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't copy files from container to host.");

            // $ docker container rm lib_tezos
            Command::new("docker")
                .args(&["container", "rm", "lib_tezos"])
                .status()
                .expect("Couldn't remove the docker container.");
        }
        "remote" => {
            std::fs::create_dir_all("lib_tezos/artifacts").expect("Failed to create directory");

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

fn update_git_submodules() {
    // pushd lib_tezos && git submodule update --init --remote src && popd
    Command::new("git")
        .args(&["submodule", "update", "--init", "--remote"])
        .current_dir(Path::new("lib_tezos").as_os_str().to_str().unwrap())
        .status()
        .expect("Couldn't update git submodules.");
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
    let want_to_update_git_submodules : bool = env::var("UPDATE_GIT_SUBMODULES").unwrap_or("true".to_string()).parse::<bool>().unwrap();
    if want_to_update_git_submodules {
        update_git_submodules();
    }

    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or("remote".to_string());
    run_builder(&build_chain);

    // copy artifact files to OUT_DIR location
    let out_dir = env::var("OUT_DIR").unwrap();
    let artifacts_dir_items = fs::read_dir(Path::new("lib_tezos").join("artifacts").as_path()).unwrap().filter_map(Result::ok)
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

    rerun_if_ocaml_file_changes();

    println!("cargo:rustc-link-search={}", &out_dir);
    println!("cargo:rustc-link-lib=dylib=tezos");
    println!("cargo:rerun-if-env-changed=OCAML_LIB");
    println!("cargo:rerun-if-env-changed=UPDATE_GIT_SUBMODULES");
}
