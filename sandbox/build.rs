// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::fs::File;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use colored::*;
use os_type::{current_platform, OSType};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const GIT_RELEASE_DISTRIBUTIONS_FILE: &str = "../tezos/interop/lib_tezos/libtezos-ffi-distribution-summary.json";
const ARTIFACTS_DIR: &str = "artifacts";
const TEZOS_CLIENT_DIR: &str = "../light_node/etc/tezedge_sandbox/tezos-client";

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
        OSType::OSX => Some("tezos-client-macos"),
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some("tezos-client-ubuntu16"),
            "18.04" | "18.10" => Some("tezos-client-ubuntu18"),
            "19.04" | "19.10" => Some("tezos-client-ubuntu19"),
            "20.04" | "20.10" => Some("tezos-client-ubuntu20"),
            _ => None,
        }
        OSType::Debian => match platform.version.as_str() {
            "9" => Some("tezos-client-debian9"),
            v if v.starts_with("9.") => Some("tezos-client-debian9"),
            "10" => Some("tezos-client-debian10"),
            v if v.starts_with("10.") => Some("tezos-client-debian10"),
            _ => None
        }
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some("tezos-client-opensuse15.1"),
            v if v.len() == 8 && v.chars().all(char::is_numeric) => Some("tezos-client-opensuse_tumbleweed"),
            _ => None
        }
        OSType::CentOS => match platform.version.chars().next().unwrap() {
            '6' => {
                println!("cargo:warning=CentOS 6.x is not supported by the OCaml Package Manager");
                None
            }
            '7' => Some("tezos-client-centos7"),
            '8' => Some("tezos-client-centos8"),
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
                        .unwrap_or_else(|| panic!("Expected artifact for name: '{}', artifacts: {:?}", &artifact_for_platform_sha256, artifacts));

                    RemoteLib {
                        lib_url: artifact.url.to_string(),
                        sha256_checksum_url: artifact_sha256.url.to_string(),
                    }
                }
                None => {
                    println!("cargo:warning=No precompiled library found for '{:?}'.", platform);
                    println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/simplestaking/tezos-opam-builder".bright_white());
                    panic!("No precompiled library");
                }
            }
        }
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
        .unwrap_or_else(|_| panic!("Couldn't read current distributions artifacts from file: {:?}", &GIT_RELEASE_DISTRIBUTIONS_FILE));
    artifacts
}

fn run_builder(build_chain: &str) {
    match build_chain {
        "local" => {
            println!("Resolved build_chain is local, so we expect that you have already tezos-client binary");
        }
        "remote" => {
            let libtezos_path = Path::new("artifacts").join("tezos-client");
            let remote_lib = get_remote_lib();
            println!("Resolved platform-dependent remote_lib: {:?}", &remote_lib);

            // get library: $ curl <remote_url> --output lib_tezos/artifacts/tezos-client-*
            Command::new("curl")
                .args(&[remote_lib.lib_url.as_str(), "--output", libtezos_path.as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't retrieve compiled tezos binary.");

            // get sha256 checksum file: $ curl <remote_url>
            let remote_lib_sha256: Output = Command::new("curl")
                .args(&[remote_lib.sha256_checksum_url.as_str()])
                .output()
                .unwrap_or_else(|_| panic!("Couldn't retrieve sha256check file for tezos binary from url: {:?}!", remote_lib.sha256_checksum_url.as_str()));
            let remote_lib_sha256 = hex::decode(std::str::from_utf8(&remote_lib_sha256.stdout).expect("Invalid UTF-8 value!")).expect("Invalid hex value!");

            // check sha256 hash
            {
                let mut file = File::open(&libtezos_path).expect("Failed to read contents of libtezos.so");
                // need to set executable persissions
                if let Err(e) = file.set_permissions(fs::Permissions::from_mode(0o722)) {
                    eprintln!("Failed to set executable permissions for {:?}, Reason: {:?}", &file, e);
                }
                let mut sha256 = Sha256::new();
                std::io::copy(&mut file, &mut sha256).expect("Failed to read contents of libtezos.so");
                let hash = sha256.finalize();
                assert_eq!(hash[..], *remote_lib_sha256, "libtezos.so SHA256 mismatch");
            }
        }
        _ => {
            println!("cargo:warning=Invalid OCaml build chain '{}'.", build_chain);
            unimplemented!("Invalid OCaml build chain");
        }
    };
}

fn main() {
    // ensure lib_tezos/artifacts directory is empty
    if Path::new(ARTIFACTS_DIR).exists() {
        fs::remove_dir_all(ARTIFACTS_DIR).expect("Failed to delete artifacts directory!");
    }
    fs::create_dir_all(ARTIFACTS_DIR).expect("Failed to create artifacts directory!");

    if Path::new(TEZOS_CLIENT_DIR).exists() {
        fs::remove_dir_all(TEZOS_CLIENT_DIR).expect("Failed to delete tezos-client directory!");
    }
    fs::create_dir_all(TEZOS_CLIENT_DIR).expect("Failed to create tezos-client directory!");

    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or_else(|_| "remote".to_string());
    run_builder(&build_chain);
}
