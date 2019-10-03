use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use colored::*;
use hex_literal::hex;
use os_type::{current_platform, OSType};
use sha2::{Digest, Sha256};

const GIT_REPO_URL: &str = "https://gitlab.com/simplestaking/tezos.git";
const GIT_COMMIT_HASH: &str = "6cdd4ff984f0c7bed07f6ba19c257aba2b89da48";
const GIT_REPO_DIR: &str = "lib_tezos/src";

const ARTIFACTS_DIR: &str = "lib_tezos/artifacts";
const OPAM_CMD: &str = "opam";

struct RemoteLib {
    url: &'static str,
    sha256: [u8; 32],
}

fn get_remote_lib() -> RemoteLib {
    let platform = current_platform();
    let url = match platform.os_type {
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/adbc14e473dee8d52af75f24f5101f76/libtezos-ffi-ubuntu16.so", sha256: hex!("0c73183a048662df7a73012702c80a2590e1e3bed56d1a64778218c7cc898a8d") }),
            "18.04" | "18:10" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/f3c5fc4c424943e13765b30245a6317e/libtezos-ffi-ubuntu18.so", sha256: hex!("5980f886d4cd9b4b603c423ee77ea852a14f90274f9820a683e05edab06beeae")}),
            "19.04" | "19.10" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/cdbfaf2ef75b6de53878075988f3c799/libtezos-ffi-ubuntu19.so", sha256: hex!("2f0f7d3f1f6365ef9b2c0e34bc8cc657384375bc140257e81dce36367eda6748")}),
            _ => None,
        }
        OSType::Debian => match platform.version.as_str() {
            "9" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/e82b77c70bf55c80761f50ec6780b0c4/libtezos-ffi-debian9.so", sha256: hex!("345c75a6473e7ccd48ee4304789cfe9be5ed8455ec1df9733ee32293a49227df")}),
            "10" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/9ebc227e0f76095414fbef936f47fc8e/libtezos-ffi-debian10.so", sha256: hex!("16134611b74fcf76d386e399cec633d638c499e84fee08d800f7a22e2e0abcf1")}),
            _ => None
        }
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some(RemoteLib { url: "https://gitlab.com/simplestaking/tezos/uploads/2f64d1e9418596df8ac0f93eabeace97/libtezos-ffi-opensuse15.1.so", sha256: hex!("0674a49f92b81c6d8e944153276626ba272484969a59e44a7f6a2ce2c3f9e482")}),
            _ => None
        }
        _ => None,
    };

    match url {
        Some(url) => url,
        None => {
            println!("cargo:warning=No precompiled library found for '{:?}'.", platform);
            println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/simplestaking/tezos-opam-builder".bright_white());
            panic!("No precompiled library");
        }
    }
}

fn run_builder(build_chain: &str) {

    match build_chain.as_ref() {
        "local" => {
            // check we want to update git updates or just skip updates, because of development process and changes on ocaml side, which are not yet in git
            let update_git = env::var("UPDATE_GIT").unwrap_or("true".to_string()).parse::<bool>().unwrap();
            if update_git {
                update_git_repository();
            }

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
            let libtezos_path = Path::new("lib_tezos").join("artifacts").join("libtezos.o");
            let remote_lib = get_remote_lib();
            Command::new("curl")
                .args(&[remote_lib.url, "--output", libtezos_path.as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't retrieve compiled tezos binary.");

            // check sha256 hash
            {
                let mut file = File::open(&libtezos_path).expect("Failed to read contents of libtezos.o");
                let mut sha256 = Sha256::new();
                std::io::copy(&mut file, &mut sha256).expect("Failed to read contents of libtezos.o");
                let hash = sha256.result();
                assert_eq!(hash[..], remote_lib.sha256, "libtezos.o SHA256 mismatch");
            }

            // $ pushd lib_tezos/artifacts && ar qs libtezos.a libtezos.o && popd
            Command::new("ar")
                .args(&["qs", "libtezos.a", "libtezos.o"])
                .current_dir(Path::new("lib_tezos").join("artifacts").as_os_str().to_str().unwrap())
                .status()
                .expect("Couldn't run ar.");
        }
        _ => {
            println!("cargo:warning=Invalid OCaml build chain '{}'.", build_chain);
            unimplemented!("Invalid OCaml build chain");
        }
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
    // ensure lib_tezos/artifacts directory is empty
    if Path::new(ARTIFACTS_DIR).exists() {
        fs::remove_dir_all(ARTIFACTS_DIR).expect("Failed to delete artifacts directory!");
    }
    fs::create_dir_all(ARTIFACTS_DIR).expect("Failed to create artifacts directory!");

    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or("remote".to_string());
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
