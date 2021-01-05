// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use slog::{info, Logger};

use tezos_identity::{load_identity, store_identity, Identity, IdentityError};

/// Ensures (load or create) identity exists according to the configuration
pub fn ensure_identity(
    identity_cfg: &crate::configuration::Identity,
    log: &Logger,
) -> Result<Identity, IdentityError> {
    let identity = if identity_cfg.identity_json_file_path.exists() {
        load_identity(&identity_cfg.identity_json_file_path)
    } else {
        info!(log, "Generating new tezos identity. This will take a while"; "expected_pow" => identity_cfg.expected_pow);
        let identity = Identity::generate(identity_cfg.expected_pow);
        info!(log, "Identity successfully generated");

        match store_identity(&identity_cfg.identity_json_file_path, &identity) {
            Ok(()) => {
                info!(log, "Generated identity stored to file"; "file" => identity_cfg.identity_json_file_path.clone().into_os_string().into_string().unwrap());
                Ok(identity)
            }
            Err(e) => Err(e),
        }
    };

    identity.and_then(|identity| match identity.check_peer_id() {
        Ok(_) => Ok(identity),
        Err(e) => Err(e),
    })
}
