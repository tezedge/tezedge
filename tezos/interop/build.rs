// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use colored::*;
use os_type::{current_platform, OSType};
use serde::Deserialize;
use serde_json;
use sha2::{Digest, Sha256};

const GIT_REPO_URL: &str = "https://gitlab.com/simplestaking/tezos.git";
const GIT_COMMIT_HASH: &str = "4930055c1b1bc95cdb64db4e231fe19fbcdd1cf6";
const GIT_RELEASE_DISTRIBUTIONS_FILE: &str = "lib_tezos/libtezos-ffi-distribution-summary.json";
const GIT_REPO_DIR: &str = "lib_tezos/src";

const ARTIFACTS_DIR: &str = "lib_tezos/artifacts";
const OPAM_CMD: &str = "opam";

#[derive(Debug)]
struct RemoteLib {
    lib_url: String,
    sha256_checksum_url: String,
}

#[derive(Deserialize, Debug)]
struct Artifact {
    name: String,
    url: String,
}

fn get_remote_lib() -> RemoteLib {
    let platform = current_platform();
    let artifacts = current_release_distributions_artifacts();
    println!("Resolved known artifacts: {:?}", &artifacts);

    let artifact_for_platform = match platform.os_type {
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some("libtezos-ffi-ubuntu16.so"),
            "18.04" | "18.10" => Some("libtezos-ffi-ubuntu18.so"),
            "19.04" | "19.10" => Some("libtezos-ffi-ubuntu19.so"),
            _ => None,
        }
        OSType::Debian => match platform.version.as_str() {
            "9" => Some("libtezos-ffi-debian9.so"),
            "10" => Some("libtezos-ffi-debian10.so"),
            _ => None
        }
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some("libtezos-ffi-opensuse15.1.so"),
            _ => None
        }
        OSType::CentOS => match platform.version.chars().next().unwrap() {
            '6' => {
                println!("cargo:warning=CentOS 6.x is not supported by the OCaml Package Manager");
                None
            }
            '7' => Some("libtezos-ffi-centos7.so"),
            '8' => Some("libtezos-ffi-centos8.so"),
            _ => None
        }
        _ => None,
    };

    match artifact_for_platform {
        Some(artifact_for_platform) => {
            // find artifact for platform
            match artifacts.iter().find(|a| a.name == artifact_for_platform) {
                Some(artifact) => {
                    let artifact_for_platform_sha256 = format!("{}.sha256", &artifact_for_platform);
                    let artifact_sha256 = artifacts
                        .iter()
                        .find(|a| a.name.as_str() == artifact_for_platform_sha256.as_str())
                        .expect(&format!("Expected artifact for name: '{}', artifacts: {:?}", &artifact_for_platform_sha256, artifacts));

                    RemoteLib {
                        lib_url: artifact.url.to_string(),
                        sha256_checksum_url: artifact_sha256.url.to_string(),
                    }
                },
                None => {
                    println!("cargo:warning=No precompiled library found for '{:?}'.", platform);
                    println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/simplestaking/tezos-opam-builder".bright_white());
                    panic!("No precompiled library");
                }
            }
        },
        None => {
            println!("cargo:warning=Not yet supported platform: '{:?}', requested artifact_for_platform: {:?}!", platform, artifact_for_platform);
            println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/simplestaking/tezos-opam-builder".bright_white());
            panic!("Not yet supported platform!");
        }
    }
}

fn current_release_distributions_artifacts() -> Vec<Artifact> {
    let artifacts: Vec<Artifact> = fs::read_to_string(PathBuf::from(GIT_RELEASE_DISTRIBUTIONS_FILE))
        .map(|output| {
            serde_json::from_str::<Vec<Artifact>>(output.as_str()).unwrap()
        })
        .expect(&format!("Couldn't read current distributions artifacts from file: {:?}", &GIT_RELEASE_DISTRIBUTIONS_FILE));
    artifacts
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
            let libtezos_path = Path::new("lib_tezos").join("artifacts").join("libtezos.so");
            let remote_lib = get_remote_lib();
            println!("Resolved platform-dependent remote_lib: {:?}", &remote_lib);

            // get library: $ curl <remote_url> --output lib_tezos/artifacts/libtezos.so
            Command::new("curl")
                .args(&[remote_lib.lib_url.as_str(), "--output", libtezos_path.as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't retrieve compiled tezos binary.");

            // get sha256 checksum file: $ curl <remote_url>
            let remote_lib_sha256: Output = Command::new("curl")
                .args(&[remote_lib.sha256_checksum_url.as_str()])
                .output()
                .expect(&format!("Couldn't retrieve sha256check file for tezos binary from url: {:?}!", remote_lib.sha256_checksum_url.as_str()));
            let remote_lib_sha256 = hex::decode(std::str::from_utf8(&remote_lib_sha256.stdout).expect("Invalid UTF-8 value!")).expect("Invalid hex value!");

            // check sha256 hash
            {
                let mut file = File::open(&libtezos_path).expect("Failed to read contents of libtezos.so");
                let mut sha256 = Sha256::new();
                std::io::copy(&mut file, &mut sha256).expect("Failed to read contents of libtezos.so");
                let hash = sha256.result();
                assert_eq!(hash[..], *remote_lib_sha256, "libtezos.so SHA256 mismatch");
            }
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
        .filter(|(_, file_name)| file_name.ends_with("ml") || file_name.ends_with("mli"))
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
    println!("cargo:rustc-link-lib=dylib=tezos_interop_callback");
    println!("cargo:rerun-if-env-changed=OCAML_LIB");
    println!("cargo:rerun-if-env-changed=UPDATE_GIT");
}
