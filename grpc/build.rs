// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     tonic_build::configure()
//         .out_dir("proto_gen")
//         .format(true)
//         .compile(&["proto/services/monitor/monitor.proto", "proto/services/chain/chain.proto"], 
//                  &["proto/services/monitor", "proto/services/chain"])
//         .expect("failed to compile protos");

//         Ok(())
// }

use std::process::Command;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        // .out_dir("proto_gen")
        .format(true)
        .compile(&["proto/tezedge.proto"], 
                 &["proto"])
        .expect("failed to compile protos");

    // Set process specific variable GIT_HASH to contain hash of current git head.
    let proc = Command::new("git").args(&["rev-parse", "HEAD"]).output();
    if let Ok(output) = proc {
        let git_hash = String::from_utf8(output.stdout).expect("Got non utf-8 response from `git rev-parse HEAD`");
        println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    }

    Ok(())
}