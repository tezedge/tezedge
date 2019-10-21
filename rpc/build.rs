use std::process::Command;

fn main() {
    let proc = Command::new("git").args(&["rev-parse", "HEAD"]).output();
    if let Ok(output) = proc {
        let git_hash = String::from_utf8(output.stdout).expect("Got non utf-8 response from `git rev-parse HEAD`");
        println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    }
}