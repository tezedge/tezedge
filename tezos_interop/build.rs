use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn run_builder(lib_dir: &PathBuf) {
    let build_chain = env::var("OCAML_BUILD_CHAIN").unwrap_or("local".to_string());

    match build_chain.as_ref() {
        "local" => {
            // $ opam config exec make
            Command::new("opam")
                .args(&["config", "exec", "make"])
                .current_dir(lib_dir)
                .status()
                .expect("Couldn't run builder. Do you have opam and dune installed on your machine?");
        }
        "docker" => {
            // $ docker build -t lib_ocaml .
            Command::new("docker")
                .args(&["build", "-t", "lib_ocaml", "."])
                .current_dir("docker")
                .status()
                .expect("Couldn't run docker build.");

            // $ docker run -di lib_ocaml sh
            let docker_run_output = String::from_utf8(Command::new("docker")
                .args(&["run", "-di", "lib_ocaml", "sh"])
                .output()
                .expect("Couldn't start docker container.")
                .stdout)
                .unwrap();
            let container_id = docker_run_output.trim();

            // $ docker cp <lib_dir> <container_id>:/home/appuser/build
            Command::new("docker")
                .args(&["cp", lib_dir.as_os_str().to_str().unwrap(), &format!("{}:/home/appuser/build", container_id)])
                .status()
                .expect("Couldn't copy files to container.");

            // $ docker exec -w /home/appuser/build <container_id> make
            Command::new("docker")
                .args(&["exec", "-w", "/home/appuser/build", container_id, "make"])
                .status()
                .expect("Couldn't run build process inside of docker container.");

            // $ docker cp <container_id>:/home/appuser/build/artifacts <lib_dir>/artifacts
            Command::new("docker")
                .args(&["cp", &format!("{}:/home/appuser/build/artifacts", container_id), lib_dir.join("artifacts").as_os_str().to_str().unwrap()])
                .status()
                .expect("Couldn't copy files from container to host.");

            // $ docker stop <container_id>
//            Command::new("docker")
//                .args(&["kill", container_id])
//                .status()
//                .expect("Couldn't stop container.");

        }
        _ => unimplemented!("cargo:warning=Invalid OCaml build chain '{}' .", build_chain)
    };
}

fn rerun_if_ocaml_file_changes(lib_dir: &PathBuf) {
    fs::read_dir(lib_dir.as_path())
        .unwrap()
        .filter_map(Result::ok)
        .map(|dir_entry| dir_entry.path())
        .filter(|path| path.is_file())
        .map(|path| (path.as_os_str().to_str().unwrap().to_string(), path.file_name().unwrap().to_str().unwrap().to_string()))
        .filter(|(_, file_name)| file_name.ends_with("ml")|| file_name.ends_with("mli"))
        .for_each(|(path, _)| println!("cargo:rerun-if-changed={}", path));
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let lib_name = env::var("OCAML_LIB").unwrap_or("tzmock".to_string());
    let lib_dir = Path::new("lib_ocaml").join(&lib_name);
    run_builder(&lib_dir);

    // copy artifact files to OUT_DIR location
    let artifacts_dir_items = fs::read_dir(lib_dir.join("artifacts").as_path()).unwrap().filter_map(Result::ok)
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

    rerun_if_ocaml_file_changes(&lib_dir);

    println!("cargo:rustc-link-search={}", &out_dir);
    println!("cargo:rustc-link-lib=dylib={}", &lib_name);
    println!("cargo:rerun-if-env-changed=OCAML_LIB");
}
