// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use redux_rs::{ActionWithMeta, Store};

use crypto::hash::ContractTz1Hash;

use super::{action::*, service::ServiceDefault, state::State};
use crate::{
    endorsement::{generate_endorsement, generate_preendorsement},
    key,
};

pub fn effects(store: &mut Store<State, ServiceDefault, Action>, action: &ActionWithMeta<Action>) {
    match &action.action {
        Action::RunWithLocalNode(RunWithLocalNodeAction {
            base_dir,
            node_dir,
            baker,
        }) => {
            let ServiceDefault { client, log } = &store.service();

            let _ = node_dir;
            let (public_key, secret_key) = key::read_key(&base_dir, baker).unwrap();
            let public_key_hash = ContractTz1Hash::try_from(public_key.clone()).unwrap();

            let chain_id = client.chain_id().unwrap();

            client.wait_bootstrapped().unwrap();

            let constants = client.constants().unwrap();
            let quorum_size = 2 * constants.consensus_committee_size / 3 + 1;

            // TODO: avoid double endorsement

            // iterating over current heads
            let heads = client.monitor_main_head().unwrap();
            for head in heads {
                let level = head.level;
                // TODO: cache it, we don't need to ask it for any round
                let rights = client.validators(level).unwrap();
                let slots = rights.iter().find_map(|v| {
                    if v.delegate == public_key_hash {
                        Some(&v.slots)
                    } else {
                        None
                    }
                });
                let slot = match slots.and_then(|v| v.first()) {
                    Some(slot) => *slot,
                    // have no rights, skip the block
                    None => continue,
                };

                let branch = head.predecessor;
                let payload_hash = hex::decode(&head.protocol_data[..64]).unwrap();
                let round_bytes = hex::decode(&head.protocol_data[64..72]).unwrap();
                let round = u32::from_be_bytes(round_bytes.try_into().unwrap());

                let op = generate_preendorsement(
                    &branch,
                    slot,
                    level,
                    round,
                    payload_hash.clone(),
                    &chain_id,
                    &secret_key,
                )
                .unwrap();
                if let Err(err) = client.inject_operation(&chain_id, &hex::encode(&op)) {
                    slog::error!(log, "{}", err);
                }

                let mut num_preendorsement = 0;
                let operations = client.monitor_operations().unwrap();
                for operations_batch in operations {
                    let operations_batch = operations_batch.as_array().unwrap();
                    for operation in operations_batch {
                        let operation_obj = operation.as_object().unwrap();
                        let this_branch = operation_obj.get("branch").unwrap().as_str().unwrap();
                        if this_branch != branch.to_base58_check() {
                            continue;
                        }
                        let contents = operation_obj.get("contents").unwrap().as_array().unwrap();
                        for content in contents {
                            let content_obj = content.as_object().unwrap();
                            let kind = content_obj.get("kind").unwrap().as_str().unwrap();
                            if kind != "preendorsement" {
                                continue;
                            }
                            let payload_hash_str = content_obj
                                .get("block_payload_hash")
                                .unwrap()
                                .as_str()
                                .unwrap();
                            // TODO: payload hash base58 compare
                            let _ = payload_hash_str;

                            let this_slot =
                                content_obj.get("slot").unwrap().as_u64().unwrap() as u16;

                            for rights_entry in &rights {
                                if rights_entry.slots.contains(&this_slot) {
                                    num_preendorsement += rights_entry.slots.len() as u32;
                                }
                            }
                        }
                    }
                    if num_preendorsement >= quorum_size {
                        let op = generate_endorsement(
                            &branch,
                            slot,
                            level,
                            round,
                            payload_hash,
                            &chain_id,
                            &secret_key,
                        )
                        .unwrap();
                        client
                            .inject_operation(&chain_id, &hex::encode(&op))
                            .unwrap();
                        break;
                    }
                }
            }
        }
        _ => {}
    }
}
