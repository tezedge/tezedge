use std::env;
use std::fs;
use std::path::Path;

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let path = Path::new(out_dir.as_str())
        .join(Path::new(dir_name))
        .to_path_buf();
    if path.exists() {
        fs::remove_dir_all(&path).expect(&format!("Failed to delete directory: {:?}", &path));
    }
    fs::create_dir_all(&path).expect(&format!("Failed to create directory: {:?}", &path));
    String::from(path.to_str().unwrap())
}