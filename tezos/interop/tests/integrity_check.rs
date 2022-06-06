// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::str::FromStr;

use tezos_interop::apply_encoded_message;
use tezos_protocol_ipc_messages::{IntegrityCheckContextRequest, NodeMessage, ProtocolMessage};

#[test]
fn test_integrity_check() {
    let context_path = match std::env::var("IRMIN_INTEGRITY_CHECK_PATH") {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Irmin path not found, ignoring test");
            return;
        }
    };

    let auto_repair: bool = std::env::var("IRMIN_INTEGRITY_CHECK_AUTO_REPAIR")
        .ok()
        .and_then(|s| FromStr::from_str(s.as_str()).ok())
        .unwrap_or(false);

    let result = apply_encoded_message(ProtocolMessage::IntegrityCheckContext(
        IntegrityCheckContextRequest {
            context_path,
            auto_repair,
        },
    ))
    .unwrap();

    eprintln!("Result={:?}", result);
    assert!(matches!(
        result,
        NodeMessage::IntegrityCheckContextResponse(Ok(()))
    ));
}
