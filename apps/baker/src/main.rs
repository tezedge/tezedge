// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
use self::command_line::{Arguments, Command};

mod logger;

mod client;
use self::client::TezosClient;

mod types;

mod key;

mod machine;
use self::machine::{action::*, effects, reducer, service::ServiceDefault, state::State};

fn main() {
    use std::time::SystemTime;

    let Arguments {
        base_dir,
        endpoint,
        log_requests,
        command,
    } = Arguments::from_args();

    // std::process::Command::new("cp").arg("-R").arg(&base_dir).arg("/home/vscode/workspace/tezedge/target/d").output().unwrap();

    let log = logger::logger(false, slog::Level::Info);
    let requests_logger = if log_requests {
        log.clone()
    } else {
        logger::logger(true, slog::Level::Info)
    };
    let (client, _) = TezosClient::new(requests_logger, endpoint);

    let service = ServiceDefault { log, client };
    let initial_time = SystemTime::now();
    let initial_state = State::default();

    let mut store = redux_rs::Store::new(reducer, effects, service, initial_time, initial_state);
    match command {
        Command::RunWithLocalNode { node_dir, baker } => {
            store.dispatch(RunWithLocalNodeAction {
                base_dir,
                node_dir,
                baker,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::types::generate_preendorsement;

    use crypto::hash::{BlockHash, BlockPayloadHash, ChainId, SeedEd25519};

    #[test]
    fn endorsement_encoding_signing() {
        let branch_str = "BM9cu8SshLv56WXMNWnZv7qEqcjzoZPGrkbhYPFbS9jcPxQPfJu";
        let branch = BlockHash::from_base58_check(branch_str).unwrap();

        let payload_hash_str = "a9d4e49a39e397fea1a11bc3e358d954a45cea0d01ffeb735bc9ddc0587d17c8";
        let payload_hash = BlockPayloadHash(hex::decode(payload_hash_str).unwrap());

        let chain_id_str = "NetXdQprcVkpaWU";
        let chain_id = ChainId::from_base58_check(chain_id_str).unwrap();

        let seed_str = "edsk39qAm1fiMjgmPkw1EgQYkMzkJezLNewd7PLNHTkr6w9XA2zdfo";
        let seed = SeedEd25519::from_base58_check(seed_str).unwrap();
        let (_, sk) = seed.keypair().unwrap();

        let op = generate_preendorsement(&branch, 1, 13, 0, payload_hash, &chain_id, &sk).unwrap();
        let op_str = "bd3a412d321077a1361610f8ed0f04b92ce8004e7f81bc29db32b1af719201b91400010000000d00000000a9d4e49a39e397fea1a11bc3e358d954a45cea0d01ffeb735bc9ddc0587d17c8e1cc355e59f33968353ed368a8353e5f920bb94f17e6f8b502694d6f0b1b167ff9b0450bf8cd67914f28de1cd31ab8a25f252fe4122d084322b8d29a8eb0a004";

        assert_eq!(hex::encode(&op), op_str);
    }
}
