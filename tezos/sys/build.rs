// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use colored::*;
use flate2::bufread::GzDecoder;
use os_type::{current_platform, OSType};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const GIT_RELEASE_DISTRIBUTIONS_FILE: &str = "lib_tezos/libtezos-ffi-distribution-summary.json";
const LIBTEZOS_BUILD_NAME: &str = "libtezos-ffi.a";
const ARTIFACTS_DIR: &str = "lib_tezos/artifacts";

// zcash params files for sapling - these files are fixed and never ever changes
const ZCASH_PARAM_SAPLING_SPEND_FILE_NAME: &str = "sapling-spend.params";
const ZCASH_PARAM_SAPLING_OUTPUT_FILE_NAME: &str = "sapling-output.params";
const ZCASH_PARAM_SAPLING_FILES_TEZOS_PATH: &str = "_opam/share/zcash-params";

#[derive(Debug)]
struct RemoteFile {
    file_url: String,
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

fn get_remote_lib(artifacts: &[Artifact]) -> RemoteFile {
    let platform = current_platform();

    let artifact_for_platform = match platform.os_type {
        OSType::OSX => Some("libtezos-ffi-macos.a.gz"),
        OSType::Ubuntu => match platform.version.as_str() {
            "16.04" => Some("libtezos-ffi-ubuntu16.a.gz"),
            "18.04" | "18.10" => Some("libtezos-ffi-ubuntu18.a.gz"),
            "19.04" | "19.10" => Some("libtezos-ffi-ubuntu19.a.gz"),
            "20.04" | "20.10" => Some("libtezos-ffi-ubuntu20.a.gz"),
            "21.04" | "21.10" => Some("libtezos-ffi-ubuntu21.a.gz"),
            _ => None,
        },
        OSType::Debian => match platform.version.as_str() {
            "9" => Some("libtezos-ffi-debian9.a.gz"),
            v if v.starts_with("9.") => Some("libtezos-ffi-debian9.a.gz"),
            "10" => Some("libtezos-ffi-debian10.a.gz"),
            v if v.starts_with("10.") => Some("libtezos-ffi-debian10.a.gz"),
            _ => None,
        },
        OSType::OpenSUSE => match platform.version.as_str() {
            "15.1" | "15.2" => Some("libtezos-ffi-opensuse15.1.a.gz"),
            v if v.len() == 8 && v.chars().all(char::is_numeric) => {
                Some("libtezos-ffi-opensuse_tumbleweed.a.gz")
            }
            _ => None,
        },
        OSType::CentOS => match platform.version.chars().next().unwrap() {
            '6' => {
                println!("cargo:warning=CentOS 6.x is not supported by the OCaml Package Manager");
                None
            }
            '7' => Some("libtezos-ffi-centos7.a.gz"),
            '8' => Some("libtezos-ffi-centos8.a.gz"),
            _ => None,
        },
        _ => None,
    };

    match artifact_for_platform {
        Some(artifact_for_platform) => {
            // find artifact for platform
            match get_remote_file(artifact_for_platform, artifacts) {
                Some(artifact) => {
                    println!("Resolved platform-dependent remote_lib: {:?}", &artifact);
                    artifact
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
            println!("cargo:warning=Not yet supported platform: '{:?}', requested artifact_for_platform: {:?}!", platform, artifact_for_platform);
            println!("{}", "To add support for your platform create a PR or open a new issue at https://github.com/tezedge/tezos-opam-builder".bright_white());
            panic!("Not yet supported platform!");
        }
    }
}

fn get_remote_file(artifact_name: &str, artifacts: &[Artifact]) -> Option<RemoteFile> {
    // find artifact for platform
    artifacts
        .iter()
        .find(|a| a.name == artifact_name)
        .map(|artifact| {
            let artifact_sha256 = format!("{}.sha256", &artifact_name);
            let artifact_sha256 = artifacts
                .iter()
                .find(|a| a.name.as_str() == artifact_sha256.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "Expected artifact for name: '{}', artifacts: {:?}",
                        &artifact_sha256, artifacts
                    )
                });

            RemoteFile {
                file_url: artifact.url.to_string(),
                sha256_checksum_url: artifact_sha256.url.to_string(),
            }
        })
}

fn download_remote_file_and_check_sha256(remote_file: RemoteFile, dest_path: &PathBuf) {
    // get file: $ curl <remote_url> --output <dest_path>
    Command::new("curl")
        .args(&[
            "-L",
            remote_file.file_url.as_str(),
            "--output",
            dest_path.as_os_str().to_str().unwrap(),
        ])
        .status()
        .unwrap_or_else(|_| {
            panic!(
                "Couldn't download file '{}' to '{:?}'",
                remote_file.file_url.as_str(),
                dest_path
            )
        });

    // get sha256 checksum file: $ curl <remote_url>
    let remote_file_sha256: Output = Command::new("curl")
        .args(&["-L", remote_file.sha256_checksum_url.as_str()])
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "Couldn't retrieve sha256check file for tezos binary from url: {:?}!",
                remote_file.sha256_checksum_url.as_str()
            )
        });
    let remote_file_sha256 =
        hex::decode(std::str::from_utf8(&remote_file_sha256.stdout).expect("Invalid UTF-8 value!"))
            .expect("Invalid hex value!");

    // check sha256 hash
    {
        let mut file = File::open(dest_path)
            .unwrap_or_else(|_| panic!("Failed to read contents of {:?}", dest_path));
        let mut sha256 = Sha256::new();
        std::io::copy(&mut file, &mut sha256)
            .unwrap_or_else(|_| panic!("Failed to copy to sha256 contents of {:?}", dest_path));
        let hash = sha256.finalize();
        assert_eq!(
            hash[..],
            *remote_file_sha256,
            "{:?} vs {:?} SHA256 mismatch",
            remote_file.file_url,
            remote_file.sha256_checksum_url
        );
    }
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

fn download_remote_file_and_check_sha256_and_uncompress(
    remote_file: RemoteFile,
    dest_path: &PathBuf,
) {
    let compressed_name = format!("{}.gz", dest_path.to_str().unwrap());
    download_remote_file_and_check_sha256(remote_file, &PathBuf::from(compressed_name.clone()));
    gunzip_file(dest_path);
}

fn libtezos_filename() -> &'static str {
    let platform = current_platform();
    match platform.os_type {
        OSType::OSX => "libtezos.a",
        _ => "libtezos.a",
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
    let artifacts_path = Path::new(ARTIFACTS_DIR);
    let libtezos_ffi_dst_path = artifacts_path.join(libtezos_filename());
    let zcash_params_spend_dest_path = artifacts_path.join(ZCASH_PARAM_SAPLING_SPEND_FILE_NAME);
    let zcash_params_output_dest_path = artifacts_path.join(ZCASH_PARAM_SAPLING_OUTPUT_FILE_NAME);

    match build_chain {
        BuildChain::Local(tezos_base_dir) => {
            let tezos_path = Path::new(&tezos_base_dir);
            let libtezos_ffi_src_path = tezos_path.join(LIBTEZOS_BUILD_NAME);
            let zcash_params_spend_src_path = tezos_path
                .join(ZCASH_PARAM_SAPLING_FILES_TEZOS_PATH)
                .join(ZCASH_PARAM_SAPLING_SPEND_FILE_NAME);
            let zcash_params_output_src_path = tezos_path
                .join(ZCASH_PARAM_SAPLING_FILES_TEZOS_PATH)
                .join(ZCASH_PARAM_SAPLING_OUTPUT_FILE_NAME);

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

            if !zcash_params_spend_src_path.exists() {
                println!(
                    "{} {} was not found!",
                    "error".bright_red(),
                    zcash_params_spend_src_path.to_str().unwrap()
                );

                println!();
                println!(
                    "Please build Tezos repo before continuing (see: ./tezos/interop/README.md)."
                );
                panic!();
            }

            if !zcash_params_output_src_path.exists() {
                println!(
                    "{} {} was not found!",
                    "error".bright_red(),
                    zcash_params_output_src_path.to_str().unwrap()
                );

                println!();
                println!(
                    "Please build Tezos repo before continuing (see: ./tezos/interop/README.md)."
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
                .unwrap_or_else(|_| {
                    panic!(
                        "Couldn't copy '{:?}' to '{:?}'",
                        libtezos_ffi_src_path, libtezos_ffi_dst_path
                    )
                });

            Command::new("cp")
                .args(&[
                    "-f",
                    zcash_params_spend_src_path.to_str().unwrap(),
                    zcash_params_spend_dest_path.to_str().unwrap(),
                ])
                .status()
                .unwrap_or_else(|_| {
                    panic!(
                        "Couldn't copy '{:?}' to '{:?}'",
                        zcash_params_spend_src_path, zcash_params_spend_dest_path
                    )
                });

            Command::new("cp")
                .args(&[
                    "-f",
                    zcash_params_output_src_path.to_str().unwrap(),
                    zcash_params_output_dest_path.to_str().unwrap(),
                ])
                .status()
                .unwrap_or_else(|_| {
                    panic!(
                        "Couldn't copy '{:?}' to '{:?}'",
                        zcash_params_output_src_path, zcash_params_output_dest_path
                    )
                });
        }
        BuildChain::Remote => {
            let artifacts = current_release_distributions_artifacts();
            println!("Resolved known artifacts: {:?}", &artifacts);

            download_remote_file_and_check_sha256_and_uncompress(
                get_remote_lib(&artifacts),
                &libtezos_ffi_dst_path,
            );
            download_remote_file_and_check_sha256(
                get_remote_file(ZCASH_PARAM_SAPLING_SPEND_FILE_NAME, &artifacts).unwrap_or_else(
                    || {
                        panic!(
                            "Failed to find file {} in artifacts",
                            ZCASH_PARAM_SAPLING_SPEND_FILE_NAME
                        )
                    },
                ),
                &zcash_params_spend_dest_path,
            );
            download_remote_file_and_check_sha256(
                get_remote_file(ZCASH_PARAM_SAPLING_OUTPUT_FILE_NAME, &artifacts).unwrap_or_else(
                    || {
                        panic!(
                            "Failed to find file {} in artifacts",
                            ZCASH_PARAM_SAPLING_OUTPUT_FILE_NAME
                        )
                    },
                ),
                &zcash_params_output_dest_path,
            );
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
    println!("cargo:rustc-link-lib=static=tezos");
    println!("cargo:rustc-link-lib=dylib=gmp");
    println!("cargo:rustc-link-lib=dylib=ffi");
    println!("cargo:rustc-link-lib=dylib=ev");
    if current_platform().os_type != OSType::OSX {
        println!("cargo:rustc-link-lib=dylib=rt");
    }
    println!("cargo:rerun-if-env-changed=TEZOS_BASE_DIR");
}
