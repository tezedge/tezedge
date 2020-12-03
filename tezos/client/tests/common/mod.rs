// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let path = test_storage_dir_path(dir_name);
    if path.exists() {
        fs::remove_dir_all(&path)
            .unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
    }
    fs::create_dir_all(&path).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
    String::from(path.to_str().unwrap())
}

pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let path = Path::new(out_dir.as_str())
        .join(Path::new(dir_name))
        .to_path_buf();
    path
}

pub fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap()
}
