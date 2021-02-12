use std::process::Command;

fn main() {
    // Set process specific variable GIT_HASH to contain hash of current git head.
    let proc = Command::new("git").args(&["rev-parse", "HEAD"]).output();
    if let Ok(output) = proc {
        let git_hash = String::from_utf8(output.stdout)
            .expect("Got non utf-8 response from `git rev-parse HEAD`");
        println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    }

    // Set process specific variable GIT_COMMIT_DATE to contain the timestamp of current git head
    let proc = Command::new("git")
        .args(&["show", "-s", "--format=%ci", "HEAD"])
        .output();
    if let Ok(output) = proc {
        let git_commit_date = String::from_utf8(output.stdout)
            .expect("Got non utf-8 response from `git show -s --format=%ci HEAD`");
        println!("cargo:rustc-env=GIT_COMMIT_DATE={}", git_commit_date);
    }
}
