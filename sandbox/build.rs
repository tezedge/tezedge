// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use colored::*;
use flate2::bufread::GzDecoder;
use os_type::{current_platform, OSType};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const GIT_RELEASE_DISTRIBUTIONS_FILE: &str =
    "../tezos/sys/lib_tezos/libtezos-ffi-distribution-summary.json";
const ARTIFACTS_DIR: &str = "artifacts";

#[derive(Debug)]
struct RemoteLib {
    name: String,
    lib_url: String,
    sha256_checksum_url: String,
}

#[derive(Deserialize, Debug)]
struct Artifact {
    name: String,
    url: String,
}

fn get_remote_libs() -> Vec<RemoteLib> {
    let platform = current_platform();
    let artifacts = current_release_distributions_artifacts();
    println!("Resolved known artifacts: {:?}", &artifacts);

    let postfix_for_platform = match platform.os_type {
        OSType::OSX => Some("macos"),
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some("ubuntu16"),
            "18.04" | "18.10" => Some("ubuntu18"),
            "19.04" | "19.10" => Some("ubuntu19"),
            "20.04" | "20.10" => Some("ubuntu20"),
            "21.04" | "21.10" => Some("ubuntu21"),
            _ => None,
        },
        OSType::Debian => match platform.version.as_str() {
            "9" => Some("debian9"),
            v if v.starts_with("9.") => Some("debian9"),
            "10" => Some("debian10"),
            v if v.starts_with("10.") => Some("debian10"),
            _ => None,
        },
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some("opensuse15.1"),
            v if v.len() == 8 && v.chars().all(char::is_numeric) => Some("opensuse_tumbleweed"),
            _ => None,
        },
        OSType::CentOS => match platform.version.chars().next().unwrap() {
            '6' => {
                println!("cargo:warning=CentOS 6.x is not supported by the OCaml Package Manager");
                None
            }
            '7' => Some("centos7"),
            '8' => Some("centos8"),
            _ => None,
        },
        _ => None,
    };

    let required_artifacts: Vec<String> = vec!["tezos-client", "tezos-admin-client"]
        .iter()
        .map(|s| s.to_string())
        .collect();

    let mut remote_libs: Vec<RemoteLib> = vec![];

    for required_artifact in required_artifacts {
        match postfix_for_platform {
            Some(postfix_for_platform) => {
                // find artifact for platform
                let artifact_for_platform =
                    format!("{}-{}.gz", required_artifact, postfix_for_platform);
                println!("Artifact to get: {}", artifact_for_platform);
                match artifacts.iter().find(|a| a.name == artifact_for_platform) {
                    Some(artifact) => {
                        let artifact_for_platform_sha256 =
                            format!("{}.sha256", &artifact_for_platform);
                        let artifact_sha256 = artifacts
                            .iter()
                            .find(|a| a.name.as_str() == artifact_for_platform_sha256.as_str())
                            .unwrap_or_else(|| {
                                panic!(
                                    "Expected artifact for name: '{}', artifacts: {:?}",
                                    &artifact_for_platform_sha256, artifacts
                                )
                            });

                        remote_libs.push(RemoteLib {
                            name: required_artifact,
                            lib_url: artifact.url.to_string(),
                            sha256_checksum_url: artifact_sha256.url.to_string(),
                        });
                    }
                    None => {
                        println!(
                            "cargo:warning=No precompiled library found for '{:?}'.",
                            platform
                        );
                        println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/tezedge/tezos-opam-builder".bright_white());
                        panic!("No precompiled library");
                    }
                }
            }
            None => {
                println!("cargo:warning=Not yet supported platform: '{:?}', requested artifact_for_platform: {:?}!", platform, required_artifact);
                println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/tezedge/tezos-opam-builder".bright_white());
                panic!("Not yet supported platform!");
            }
        }
    }
    remote_libs
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

fn gunzip_file(uncompressed_name: &PathBuf) {
    let compressed_name = format!("{}.gz", uncompressed_name.to_str().unwrap());
    let file = File::open(&compressed_name)
        .unwrap_or_else(|_| panic!("Couldn't open file: {}", compressed_name));
    let mut gz = GzDecoder::new(BufReader::new(file));
    let mut buf = vec![];

    gz.read_to_end(&mut buf)
        .unwrap_or_else(|err| panic!("Decompression failure '{}': {}", compressed_name, err));

    let outfile = File::create(uncompressed_name).expect(&format!(
        "Could not open {} for decompression",
        uncompressed_name.to_string_lossy()
    ));
    let mut writer = BufWriter::new(outfile);

    writer.write_all(&buf).unwrap_or_else(|err| {
        panic!(
            "Failed when writting to '{}': {}",
            uncompressed_name.to_string_lossy(),
            err
        )
    });
}

fn run_builder(build_chain: &str) {
    match build_chain {
        "local" => {
            println!("Resolved build_chain is local, so we expect that you have already tezos-client binary");
        }
        "remote" => {
            // let libtezos_path = Path::new("artifacts").join("tezos-client");
            let remote_libs = get_remote_libs();
            println!("Resolved platform-dependent remote_lib: {:?}", &remote_libs);

            for remote_lib in remote_libs {
                // get library: $ curl <remote_url> --output lib_tezos/artifacts/tezos-client-*
                let lib_path = Path::new("artifacts").join(&format!("{}.gz", remote_lib.name));
                Command::new("curl")
                    .args(&[
                        "-L",
                        remote_lib.lib_url.as_str(),
                        "--output",
                        lib_path.as_os_str().to_str().unwrap(),
                    ])
                    .status()
                    .expect("Couldn't retrieve compiled tezos binary.");

                // get sha256 checksum file: $ curl <remote_url>
                let remote_lib_sha256: Output = Command::new("curl")
                    .args(&["-L", remote_lib.sha256_checksum_url.as_str()])
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
                        File::open(&lib_path).expect("Failed to read contents of libtezos.so");
                    // need to set executable persissions
                    if let Err(e) = file.set_permissions(fs::Permissions::from_mode(0o722)) {
                        eprintln!(
                            "Failed to set executable permissions for {:?}, Reason: {:?}",
                            &file, e
                        );
                    }
                    let mut sha256 = Sha256::new();
                    std::io::copy(&mut file, &mut sha256)
                        .expect("Failed to read contents of libtezos.so");
                    let hash = sha256.finalize();
                    assert_eq!(hash[..], *remote_lib_sha256, "libtezos.so SHA256 mismatch");
                }

                // Uncompress the artifact
                let uncompressed_name = lib_path.with_extension("");
                gunzip_file(&uncompressed_name);
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

    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or_else(|_| "remote".to_string());
    run_builder(&build_chain);
}
