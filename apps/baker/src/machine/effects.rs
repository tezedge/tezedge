// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use crypto::{
    blake2b,
    hash::{BlockPayloadHash, ContractTz1Hash, NonceHash, OperationHash, OperationListHash},
};
use redux_rs::{ActionWithMeta, Store};

use super::{action::*, service::ServiceDefault, state::State};
use crate::{
    key,
    types::{generate_endorsement, generate_preendorsement},
};

pub fn effects(store: &mut Store<State, ServiceDefault, Action>, action: &ActionWithMeta<Action>) {
    match &action.action {
        Action::RunWithLocalNode(RunWithLocalNodeAction {
            base_dir,
            node_dir,
            baker,
        }) => {
            let ServiceDefault { client, log } = &store.service();
            let main_log = crate::logger::main_logger();

            let _ = node_dir;
            let (public_key, secret_key) = key::read_key(&base_dir, baker).unwrap();
            let public_key_hash = ContractTz1Hash::try_from(public_key.clone()).unwrap();

            let chain_id = client.chain_id().unwrap();

            client.wait_bootstrapped().unwrap();
            slog::info!(main_log, "bootstrapped");

            let constants = client.constants().unwrap();
            let quorum_size = 2 * constants.consensus_committee_size / 3 + 1;
            let minimal_block_delay = constants.minimal_block_delay.parse::<i64>().unwrap();
            let delay_increment_per_round =
                constants.delay_increment_per_round.parse::<i64>().unwrap();

            // avoid double endorsement
            let mut endorsed_level = 0;
            let mut endorsed_payload_hash = None::<BlockPayloadHash>;

            // iterating over current heads
            loop {
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
                        None => {
                            slog::info!(main_log, "have no slot at level: {}", level,);
                            continue;
                        }
                    };

                    let baking_rights = client.baking_rights(level, &public_key_hash).unwrap();
                    let baking_rounds = baking_rights
                        .into_iter()
                        .filter(|v| v.level == level && v.delegate == public_key_hash)
                        .map(|v| v.round);

                    let branch = head.predecessor;
                    let payload_hash =
                        BlockPayloadHash(hex::decode(&head.protocol_data[..64]).unwrap());
                    let round_bytes = hex::decode(&head.protocol_data[64..72]).unwrap();
                    let round = u32::from_be_bytes(round_bytes.try_into().unwrap());

                    slog::info!(
                        main_log,
                        "inject preendorsement, level: {}, slot: {}, round: {}",
                        level,
                        slot,
                        round,
                    );

                    // already endorsed another payload on this level
                    if let Some(endorsed_payload_hash) = &endorsed_payload_hash {
                        if endorsed_level == level && payload_hash.ne(endorsed_payload_hash) {
                            slog::warn!(
                                main_log,
                                "level: {}, already endorsed: {}, skip: {}",
                                level,
                                endorsed_payload_hash,
                                payload_hash,
                            );
                            continue;
                        }
                    }
                    endorsed_level = level;
                    endorsed_payload_hash = Some(payload_hash.clone());

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

                    // TODO: only for baking
                    let mut collected_operations = [vec![], vec![], vec![], vec![]];
                    let mut collected_hashes = Vec::new();

                    let mut num_preendorsement = 0;
                    let operations = client.monitor_operations().unwrap().flatten();
                    for operation in operations {
                        let operation_obj = operation.as_object().unwrap();
                        let this_branch = operation_obj.get("branch").unwrap().as_str().unwrap();
                        if this_branch != branch.to_base58_check() {
                            continue;
                        }
                        let contents = operation_obj.get("contents").unwrap().as_array().unwrap();
                        for content in contents {
                            let content_obj = content.as_object().unwrap();
                            let kind = content_obj.get("kind").unwrap().as_str().unwrap();
                            if kind == "endorsement" || kind == "preendorsement" {
                                collected_operations[0].push(operation.clone());
                            } else {
                                if let Some(hash) = operation_obj.get("hash") {
                                    if let Some(hash_str) = hash.as_str() {
                                        let hash =
                                            OperationHash::from_base58_check(hash_str).unwrap();
                                        collected_hashes.push(hash);
                                    }
                                }
                                collected_operations[3].push(operation.clone());
                            }
                            if kind != "preendorsement" {
                                continue;
                            }
                            let payload_hash_str = content_obj
                                .get("block_payload_hash")
                                .unwrap()
                                .as_str()
                                .unwrap();
                            if payload_hash.to_base58_check() != payload_hash_str {
                                continue;
                            }

                            let this_slot =
                                content_obj.get("slot").unwrap().as_u64().unwrap() as u16;

                            for rights_entry in &rights {
                                if rights_entry.slots.contains(&this_slot) {
                                    num_preendorsement += rights_entry.slots.len() as u32;
                                }
                            }
                        }
                        if num_preendorsement >= quorum_size {
                            slog::info!(main_log, "inject endorsement");
                            let op = generate_endorsement(
                                &branch,
                                slot,
                                level,
                                round,
                                payload_hash.clone(),
                                &chain_id,
                                &secret_key,
                            )
                            .unwrap();
                            client
                                .inject_operation(&chain_id, &hex::encode(&op))
                                .unwrap();

                            // TODO:
                            break;
                        }
                    }

                    // TODO: bake a block if have rights
                    let _ = (
                        minimal_block_delay,
                        delay_increment_per_round,
                        baking_rounds,
                    );
                    if false {
                        let operation_list_hash =
                            OperationListHash::calculate(&collected_hashes).unwrap();
                        let payload_hash =
                            BlockPayloadHash::calculate(&head.hash, 0, &operation_list_hash)
                                .unwrap();
                        let timestamp = head
                            .timestamp
                            .parse::<chrono::DateTime<chrono::Utc>>()
                            .unwrap();
                        let delta = if round == 0 { 3 } else { 2 };
                        let preapply_result = client
                            .preapply_block(
                                &secret_key,
                                &chain_id,
                                payload_hash,
                                0,
                                hex::decode("7985fafe1fb70300").unwrap(),
                                NonceHash(blake2b::digest_256(&[1, 2, 3]).unwrap()),
                                false,
                                collected_operations,
                                (timestamp.timestamp() + delta).to_string(),
                            )
                            .unwrap();
                        let _ = preapply_result;
                    }
                }
            }
        }
        _ => {}
    }
}
