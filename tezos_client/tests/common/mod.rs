use std::env;
use std::fs;

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let out_dir = env::var("OUT_DIR").unwrap();
    let path = std::path::Path::new(out_dir.as_str())
        .join(std::path::Path::new(dir_name))
        .to_path_buf();
    if path.exists() {
        fs::remove_dir_all(path.clone()).unwrap();
    }
    fs::create_dir(path.clone()).unwrap();
    String::from(path.to_str().unwrap())
}