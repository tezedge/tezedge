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
use sha2::{Digest, Sha256};

const GIT_RELEASE_DISTRIBUTIONS_FILE: &str = "lib_tezos/libtezos-ffi-distribution-summary.json";
const LIBTEZOS_BUILD_NAME: &str = "libtezos-ffi.so";
const ARTIFACTS_DIR: &str = "lib_tezos/artifacts";

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

enum BuildChain {
    Remote,
    Local(String),
}

fn get_remote_lib() -> RemoteLib {
    let platform = current_platform();
    let artifacts = current_release_distributions_artifacts();
    println!("Resolved known artifacts: {:?}", &artifacts);

    let artifact_for_platform = match platform.os_type {
        OSType::OSX => Some("libtezos-ffi-macos.dylib"),
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some("libtezos-ffi-ubuntu16.so"),
            "18.04" | "18.10" => Some("libtezos-ffi-ubuntu18.so"),
            "19.04" | "19.10" => Some("libtezos-ffi-ubuntu19.so"),
            "20.04" | "20.10" => Some("libtezos-ffi-ubuntu20.so"),
            _ => None,
        },
        OSType::Debian => match platform.version.as_str() {
            "9" => Some("libtezos-ffi-debian9.so"),
            v if v.starts_with("9.") => Some("libtezos-ffi-debian9.so"),
            "10" => Some("libtezos-ffi-debian10.so"),
            v if v.starts_with("10.") => Some("libtezos-ffi-debian10.so"),
            _ => None,
        },
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some("libtezos-ffi-opensuse15.1.so"),
            v if v.len() == 8 && v.chars().all(char::is_numeric) => {
                Some("libtezos-ffi-opensuse_tumbleweed.so")
            }
            _ => None,
        },
        OSType::CentOS => match platform.version.chars().next().unwrap() {
            '6' => {
                println!("cargo:warning=CentOS 6.x is not supported by the OCaml Package Manager");
                None
            }
            '7' => Some("libtezos-ffi-centos7.so"),
            '8' => Some("libtezos-ffi-centos8.so"),
            _ => None,
        },
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
                        .unwrap_or_else(|| {
                            panic!(
                                "Expected artifact for name: '{}', artifacts: {:?}",
                                &artifact_for_platform_sha256, artifacts
                            )
                        });

                    RemoteLib {
                        lib_url: artifact.url.to_string(),
                        sha256_checksum_url: artifact_sha256.url.to_string(),
                    }
                }
                None => {
                    println!(
                        "cargo:warning=No precompiled library found for '{:?}'.",
                        platform
                    );
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

fn libtezos_filename() -> &'static str {
    let platform = current_platform();
    match platform.os_type {
        OSType::OSX => "libtezos.dylib",
        _ => "libtezos.so",
    }
}

fn current_release_distributions_artifacts() -> Vec<Artifact> {
    let artifacts: Vec<Artifact> =
        fs::read_to_string(PathBuf::from(GIT_RELEASE_DISTRIBUTIONS_FILE))
            .map(|output| serde_json::from_str::<Vec<Artifact>>(output.as_str()).unwrap())
            .unwrap_or_else(|_| {
                panic!(
                    "Couldn't read current distributions artifacts from file: {:?}",
                    &GIT_RELEASE_DISTRIBUTIONS_FILE
                )
            });
    artifacts
}

fn run_builder(build_chain: &BuildChain) {
    match build_chain {
        BuildChain::Local(tezos_base_dir) => {
            let artifacts_path = Path::new(ARTIFACTS_DIR);
            let tezos_path = Path::new(&tezos_base_dir);
            let libtezos_ffi_src_path = tezos_path.join(LIBTEZOS_BUILD_NAME);
            let libtezos_ffi_dst_path = artifacts_path.join(libtezos_filename());

            if !tezos_path.exists() {
                println!(
                    "{} TEZOS_BASE_DIR={} was not found!",
                    "error".bright_red(),
                    tezos_base_dir
                );
                panic!()
            }

            if !libtezos_ffi_src_path.exists() {
                println!(
                    "{} {} was not found!",
                    "error".bright_red(),
                    libtezos_ffi_src_path.to_str().unwrap()
                );

                println!();
                println!(
                    "Please build libtezos-ffi before continuing (see: ./tezos/interop/README.md)."
                );
                panic!();
            }

            Command::new("cp")
                .args(&[
                    "-f",
                    libtezos_ffi_src_path.to_str().unwrap(),
                    libtezos_ffi_dst_path.to_str().unwrap(),
                ])
                .status()
                .expect("Couldn't copy libtezos-ffi.");
        }
        BuildChain::Remote => {
            let libtezos_filename = libtezos_filename();
            let libtezos_path = Path::new("lib_tezos")
                .join("artifacts")
                .join(libtezos_filename);
            let remote_lib = get_remote_lib();
            println!("Resolved platform-dependent remote_lib: {:?}", &remote_lib);

            // get library: $ curl <remote_url> --output lib_tezos/artifacts/libtezos.so
            Command::new("curl")
                .args(&[
                    remote_lib.lib_url.as_str(),
                    "--output",
                    libtezos_path.as_os_str().to_str().unwrap(),
                ])
                .status()
                .expect("Couldn't retrieve compiled tezos binary.");

            // get sha256 checksum file: $ curl <remote_url>
            let remote_lib_sha256: Output = Command::new("curl")
                .args(&[remote_lib.sha256_checksum_url.as_str()])
                .output()
                .unwrap_or_else(|_| {
                    panic!(
                        "Couldn't retrieve sha256check file for tezos binary from url: {:?}!",
                        remote_lib.sha256_checksum_url.as_str()
                    )
                });
            let remote_lib_sha256 = hex::decode(
                std::str::from_utf8(&remote_lib_sha256.stdout).expect("Invalid UTF-8 value!"),
            )
            .expect("Invalid hex value!");

            // check sha256 hash
            {
                let mut file =
                    File::open(&libtezos_path).expect("Failed to read contents of libtezos.so");
                let mut sha256 = Sha256::new();
                std::io::copy(&mut file, &mut sha256)
                    .expect("Failed to read contents of libtezos.so");
                let hash = sha256.finalize();
                assert_eq!(hash[..], *remote_lib_sha256, "libtezos.so SHA256 mismatch");
            }
        }
    };
}

fn main() {
    // ensure lib_tezos/artifacts directory is empty
    if Path::new(ARTIFACTS_DIR).exists() {
        fs::remove_dir_all(ARTIFACTS_DIR).expect("Failed to delete artifacts directory!");
    }
    fs::create_dir_all(ARTIFACTS_DIR).expect("Failed to create artifacts directory!");

    let tezos_base_dir = env::var("TEZOS_BASE_DIR").unwrap_or_else(|_| "".to_owned());
    let build_chain = if tezos_base_dir.is_empty() {
        BuildChain::Remote
    } else {
        BuildChain::Local(tezos_base_dir)
    };
    run_builder(&build_chain);

    // copy artifact files to OUT_DIR location
    let out_dir = env::var("OUT_DIR").unwrap();

    let artifacts_dir_items = fs::read_dir(ARTIFACTS_DIR)
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .collect::<Vec<PathBuf>>();
    let artifacts_dir_items: Vec<&Path> = artifacts_dir_items.iter().map(|p| p.as_path()).collect();
    let mut copy_options = fs_extra::dir::CopyOptions::new();
    copy_options.overwrite = true;
    let bytes_copied = fs_extra::copy_items(&artifacts_dir_items, &out_dir, &copy_options)
        .expect("Failed to copy artifacts to build output directory.");
    if bytes_copied == 0 {
        println!("cargo:warning=No files were found in the artifacts directory.");
        panic!("Failed to build tezos_interop artifacts.");
    }

    println!("cargo:rustc-link-search={}", &out_dir);
    println!("cargo:rustc-link-lib=dylib=tezos_interop_callback");
    println!("cargo:rustc-link-lib=dylib=tezos");
    println!("cargo:rerun-if-env-changed=TEZOS_BASE_DIR");
}
