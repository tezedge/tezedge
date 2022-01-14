// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::HashMap,
    convert::{TryFrom, TryInto},
    num::TryFromIntError,
    time::Instant,
};

use crypto::{
    blake2b::{self, Blake2bError},
    hash::HashBase58,
};
use slog::{error, trace, warn, FnValue};
use storage::{cycle_storage::CycleData, num_from_slice};
use tezos_messages::base::{
    signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash},
    ConversionError,
};

use crate::{
    rights::Delegate,
    service::RpcService,
    storage::{
        kv_block_additional_data, kv_block_header, kv_constants, kv_cycle_eras,
        kv_cycle_meta::{self, CycleKey},
    },
    Action, ActionWithMeta, Service, Store,
};

use super::{
    rights_actions::*,
    utils::{get_cycle, Position},
    EndorsingRights, EndorsingRightsRequest, EndorsingRightsRpcError, ProtocolConstants, Slot,
};

pub fn rights_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let requests = &store.state.get().rights.requests;
    let cache = &store.state.get().rights.cache;
    let log = &store.state.get().log;
    match &action.action {
        // Main entry action
        Action::RightsGetEndorsingRights(RightsGetEndorsingRightsAction { key }) => {
            if let Some((_, endorsing_rights)) = key
                .level
                .as_ref()
                .and_then(|level| cache.endorsing_rights.get(level))
                .cloned()
            {
                trace!(log, "Endorsing rights using cache"; "key" => FnValue(|_| format!("{:?}", key)));
                store.dispatch(RightsEndorsingRightsReadyAction {
                    key: key.clone(),
                    endorsing_rights,
                });
                return;
            } else if !requests.contains_key(key) {
                trace!(log, "Endorsing rights using full cycle"; "key" => FnValue(|_| format!("{:?}", key)));
                store.dispatch(RightsEndorsingRightsInitAction { key: key.clone() });
            } else {
                trace!(log, "Endorsing rights already in progress"; "key" => FnValue(|_| format!("{:?}", key)));
            }
        }

        // RPC actions
        Action::RightsRpcEndorsingRightsGet(RightsRpcEndorsingRightsGetAction { key, .. }) => {
            store.dispatch(RightsGetEndorsingRightsAction { key: key.clone() });
        }
        Action::RightsEndorsingRightsReady(RightsEndorsingRightsReadyAction {
            endorsing_rights,
            ..
        }) => {
            for rpc_id in store
                .state
                .get()
                .rights
                .rpc_requests
                .iter()
                .filter_map(
                    |(rpc_id, req_key @ key)| if key == req_key { Some(rpc_id) } else { None },
                )
                .cloned()
                .collect::<Vec<_>>()
            {
                match endorsing_rights
                    .delegate_to_slots
                    .iter()
                    .map(|(delegate, slots)| {
                        Ok((
                            SignaturePublicKeyHash::try_from(delegate.clone())?,
                            slots.clone(),
                        ))
                    })
                    .collect::<Result<_, EndorsingRightsRpcError>>()
                {
                    Ok(endorsing_rights) => store.dispatch(RightsRpcEndorsingRightsReadyAction {
                        rpc_id,
                        endorsing_rights,
                    }),

                    Err(err) => store.dispatch(RightsRpcEndorsingRightsErrorAction {
                        rpc_id,
                        error: err.into(),
                    }),
                };
                store.dispatch(RightsRpcEndorsingRightsPruneAction { rpc_id });
            }
        }
        Action::RightsRpcEndorsingRightsReady(RightsRpcEndorsingRightsReadyAction {
            rpc_id,
            endorsing_rights,
        }) => store
            .service
            .rpc()
            .respond(*rpc_id, endorsing_rights.clone()),
        Action::RightsRpcEndorsingRightsError(RightsRpcEndorsingRightsErrorAction {
            rpc_id,
            error,
        }) => store.service.rpc().respond(*rpc_id, error.clone()),

        Action::RightsEndorsingRightsInit(RightsEndorsingRightsInitAction { key }) => {
            store.dispatch(RightsEndorsingRightsGetBlockHeaderAction { key: key.clone() });
        }
        // get block header from kv store
        Action::RightsEndorsingRightsGetBlockHeader(
            RightsEndorsingRightsGetBlockHeaderAction { key },
        ) => {
            if let Some(EndorsingRightsRequest::PendingBlockHeader { .. }) = requests.get(key) {
                store.dispatch(kv_block_header::StorageBlockHeaderGetAction::new(
                    key.current_block_hash.clone(),
                ));
            }
        }
        Action::StorageBlockHeaderOk(kv_block_header::StorageBlockHeaderOkAction {
            key: HashBase58(key),
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if &rights_key.current_block_hash == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsBlockHeaderReadyAction {
                    key,
                    block_header: value.clone(),
                });
            }
        }
        Action::StorageBlockHeaderError(kv_block_header::StorageBlockHeaderErrorAction {
            key: HashBase58(key),
            error,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if &rights_key.current_block_hash == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsEndorsingRightsBlockHeaderReady(
            RightsEndorsingRightsBlockHeaderReadyAction { key, .. },
        ) => {
            if let Some(EndorsingRightsRequest::BlockHeaderReady { .. }) = requests.get(key) {
                store.dispatch(RightsEndorsingRightsGetProtocolHashAction { key: key.clone() });
            }
        }

        // get protocol hash as a part of additional block data from kv store
        Action::RightsEndorsingRightsGetProtocolHash(
            RightsEndorsingRightsGetProtocolHashAction { key, .. },
        ) => {
            if let Some(EndorsingRightsRequest::PendingProtocolHash { .. }) = requests.get(key) {
                store.dispatch(
                    kv_block_additional_data::StorageBlockAdditionalDataGetAction::new(
                        key.current_block_hash.clone(),
                    ),
                );
            }
        }
        Action::StorageBlockAdditionalDataOk(
            kv_block_additional_data::StorageBlockAdditionalDataOkAction {
                key: HashBase58(key),
                value,
            },
        ) => {
            let rights_keys: Vec<_> = requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if &rights_key.current_block_hash == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect();
            for rights_key in rights_keys {
                store.dispatch(RightsEndorsingRightsProtocolHashReadyAction {
                    key: rights_key.clone(),
                    proto_hash: value.next_protocol_hash.clone(),
                });
            }
        }
        Action::StorageBlockAdditionalDataError(
            kv_block_additional_data::StorageBlockAdditionalDataErrorAction {
                key: HashBase58(key),
                error,
            },
        ) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, _)| {
                    if &rights_key.current_block_hash == key {
                        Some(rights_key)
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsEndorsingRightsProtocolHashReady(
            RightsEndorsingRightsProtocolHashReadyAction { key, .. },
        ) => {
            if let Some(EndorsingRightsRequest::ProtocolHashReady { .. }) = requests.get(key) {
                store
                    .dispatch(RightsEndorsingRightsGetProtocolConstantsAction { key: key.clone() });
            }
        }

        // get protocol constants from kv store
        Action::RightsEndorsingRightsGetProtocolConstants(
            RightsEndorsingRightsGetProtocolConstantsAction { key },
        ) => {
            if let Some(EndorsingRightsRequest::PendingProtocolConstants { proto_hash, .. }) =
                requests.get(key)
            {
                let key = proto_hash.clone();
                store.dispatch(kv_constants::StorageConstantsGetAction::new(key));
            }
        }
        Action::StorageConstantsOk(kv_constants::StorageConstantsOkAction {
            key: HashBase58(key),
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingProtocolConstants {
                        proto_hash: data_proto_hash,
                        ..
                    } = request
                    {
                        if data_proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                match serde_json::from_str::<ProtocolConstants>(value) {
                    Ok(constants) => {
                        store.dispatch(RightsEndorsingRightsProtocolConstantsReadyAction {
                            key,
                            constants,
                        });
                    }
                    Err(err) => {
                        store.dispatch(RightsEndorsingRightsErrorAction {
                            key,
                            error: err.into(),
                        });
                    }
                }
            }
        }
        Action::StorageConstantsError(kv_constants::StorageConstantsErrorAction {
            key: HashBase58(key),
            error,
        }) => {
            let rights_keys: Vec<_> = requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingProtocolConstants {
                        proto_hash: data_proto_hash,
                        ..
                    } = request
                    {
                        if data_proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect();
            for rights_key in rights_keys {
                store.dispatch(RightsEndorsingRightsErrorAction {
                    key: rights_key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsEndorsingRightsProtocolConstantsReady(
            RightsEndorsingRightsProtocolConstantsReadyAction { key, .. },
        ) => {
            if let Some(EndorsingRightsRequest::ProtocolConstantsReady { .. }) = requests.get(key) {
                store.dispatch(RightsEndorsingRightsGetCycleErasAction { key: key.clone() });
            }
        }

        // get cycle eras from kv store
        Action::RightsEndorsingRightsGetCycleEras(RightsEndorsingRightsGetCycleErasAction {
            key,
        }) => {
            if let Some(EndorsingRightsRequest::PendingCycleEras { proto_hash, .. }) =
                requests.get(key)
            {
                let key = proto_hash.clone();
                store.dispatch(kv_cycle_eras::StorageCycleErasGetAction::new(key));
            }
        }
        Action::StorageCycleErasOk(kv_cycle_eras::StorageCycleErasOkAction {
            key: HashBase58(key),
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingCycleEras { proto_hash, .. } = request {
                        if proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsCycleErasReadyAction {
                    key,
                    cycle_eras: value.clone(),
                });
            }
        }
        Action::StorageCycleErasError(kv_cycle_eras::StorageCycleErasErrorAction {
            key: HashBase58(key),
            error,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingCycleEras { proto_hash, .. } = request {
                        if proto_hash == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsEndorsingRightsCycleErasReady(
            RightsEndorsingRightsCycleErasReadyAction { key, .. },
        ) => {
            store.dispatch(RightsEndorsingRightsGetCycleAction { key: key.clone() });
        }

        // get cycle for the requested block
        Action::RightsEndorsingRightsGetCycle(RightsEndorsingRightsGetCycleAction { key }) => {
            if let Some(EndorsingRightsRequest::PendingCycle {
                block_header,
                cycle_eras,
                protocol_constants,
                ..
            }) = requests.get(key)
            {
                let block_level = block_header.level();
                let get_cycle = get_cycle(
                    block_level,
                    key.level,
                    cycle_eras,
                    protocol_constants.preserved_cycles,
                );
                match get_cycle {
                    Ok((cycle, position)) => {
                        store.dispatch(RightsEndorsingRightsCycleReadyAction {
                            key: key.clone(),
                            cycle,
                            position,
                        })
                    }
                    Err(err) => store.dispatch(RightsEndorsingRightsErrorAction {
                        key: key.clone(),
                        error: err.into(),
                    }),
                };
            }
        }
        Action::RightsEndorsingRightsCycleReady(RightsEndorsingRightsCycleReadyAction {
            key,
            ..
        }) => {
            if let Some(EndorsingRightsRequest::CycleReady { .. }) = requests.get(key) {
                store.dispatch(RightsEndorsingRightsGetCycleDataAction { key: key.clone() });
            }
        }

        // get cycle data kv store
        Action::RightsEndorsingRightsGetCycleData(RightsEndorsingRightsGetCycleDataAction {
            key,
        }) => {
            if let Some(EndorsingRightsRequest::PendingCycleData { cycle, .. }) = requests.get(key)
            {
                let key = *cycle;
                store.dispatch(kv_cycle_meta::StorageCycleMetaGetAction::new(key));
            }
        }
        Action::StorageCycleMetaOk(kv_cycle_meta::StorageCycleMetaOkAction {
            key: CycleKey(key),
            value,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingCycleData { cycle, .. } = request {
                        if cycle == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsCycleDataReadyAction {
                    key,
                    cycle_data: value.clone(),
                });
            }
        }
        Action::StorageCycleMetaError(kv_cycle_meta::StorageCycleMetaErrorAction {
            key: CycleKey(key),
            error,
        }) => {
            for key in requests
                .iter()
                .filter_map(|(rights_key, request)| {
                    if let EndorsingRightsRequest::PendingCycleData { cycle, .. } = request {
                        if cycle == key {
                            Some(rights_key)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RightsEndorsingRightsErrorAction {
                    key,
                    error: error.clone().into(),
                });
            }
        }
        Action::RightsEndorsingRightsCycleDataReady(
            RightsEndorsingRightsCycleDataReadyAction { key, .. },
        ) => {
            store.dispatch(RightsEndorsingRightsCalculateAction { key: key.clone() });
        }

        Action::RightsEndorsingRightsCalculate(RightsEndorsingRightsCalculateAction { key }) => {
            if let Some(EndorsingRightsRequest::PendingRights {
                level,
                cycle_data,
                position,
                protocol_constants,
                start,
            }) = requests.get(key)
            {
                trace!(&store.state.get().log, "calculating endorsing rights"; "level" => level, "key" => FnValue(|_| format!("{:#?}", key)));
                let prereq_dur = action.id.duration_since(*start);

                let time = Instant::now();
                let endorsing_rights =
                    calculate_endorsing_rights(cycle_data, protocol_constants, *position);
                let dur = Instant::now() - time;

                match endorsing_rights {
                    Ok((delegate_to_slots, slot_to_delegate)) => {
                        let log = &store.state.get().log;
                        trace!(log, "Endorsing rights successfully calculated";
                               "level" => level,
                               "prerequisites duration" => FnValue(|_| prereq_dur.as_millis()),
                               "duration" => FnValue(|_| dur.as_millis())
                        );
                        let level = *level;
                        store.dispatch(RightsEndorsingRightsReadyAction {
                            key: key.clone(),
                            endorsing_rights: EndorsingRights {
                                level,
                                slot_to_delegate,
                                delegate_to_slots,
                            },
                        });
                    }
                    Err(err) => {
                        store.dispatch(RightsEndorsingRightsErrorAction {
                            key: key.clone(),
                            error: err.into(),
                        });
                    }
                }
            }
        }
        Action::RightsEndorsingRightsError(RightsEndorsingRightsErrorAction { key, error }) => {
            warn!(log, "Error getting endorsing rights"; "key" => format!("{:?}", key), "error" => error.to_string());
        }

        _ => (),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum EndorsingRightsCalculationError {
    #[error("Integer conversion error: {0}")]
    FromInt(String),
    #[error("Digest error: {0}")]
    Blake2b(#[from] Blake2bError),
    #[error("Signature conversion error: {0}")]
    Conversion(#[from] ConversionError),
    #[error("Value of bound(last_roll) `{0}` is not correct")]
    BoundNotCorrect(i32),
}

impl From<TryFromIntError> for EndorsingRightsCalculationError {
    fn from(error: TryFromIntError) -> Self {
        Self::FromInt(error.to_string())
    }
}

fn calculate_endorsing_rights(
    cycle_meta_data: &CycleData,
    constants: &ProtocolConstants,
    cycle_position: Position,
) -> Result<(HashMap<Delegate, Vec<Slot>>, Vec<Delegate>), EndorsingRightsCalculationError> {
    // build a reverse map of rols so we have access in O(1)
    let mut rolls_map = HashMap::new();

    for (delegate, rolls) in cycle_meta_data.rolls_data() {
        for roll in rolls {
            rolls_map.insert(
                *roll,
                SignaturePublicKey::from_tagged_bytes(delegate.to_vec())?,
            );
        }
    }

    let endorsers_slots =
        get_endorsers_slots(constants, cycle_meta_data, &rolls_map, cycle_position)?;

    let mut endorser_to_slots = HashMap::new();
    for (slot, delegate) in endorsers_slots.iter().enumerate() {
        endorser_to_slots
            .entry(delegate.clone())
            .or_insert_with(|| Vec::new())
            .push(slot.try_into()?);
    }
    Ok((endorser_to_slots, endorsers_slots))
}

/// Use tezos PRNG to collect all slots for each endorser by public key hash (for later ordering of endorsers)
///
/// # Arguments
///
/// * `constants` - Context constants used in baking and endorsing rights [RightsConstants](RightsConstants::parse_rights_constants).
/// * `cycle_meta_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `level` - Level to feed Tezos PRNG.
#[inline]
fn get_endorsers_slots(
    constants: &ProtocolConstants,
    cycle_meta_data: &CycleData,
    rolls_map: &HashMap<i32, Delegate>,
    cycle_position: i32,
) -> Result<Vec<Delegate>, EndorsingRightsCalculationError> {
    // special byte string used in Tezos PRNG
    const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
    // prepare helper variable
    let mut endorsers_slots: Vec<Delegate> = Vec::new();

    for endorser_slot in 0..constants.endorsers_per_block {
        // generate PRNG per endorsement slot and take delegates by roll number from context_rolls
        // if roll number is not found then reroll with new state till roll nuber is found in context_rolls
        let mut state = init_prng(
            cycle_meta_data,
            constants,
            ENDORSEMENT_USE_STRING,
            cycle_position,
            endorser_slot.into(),
        )?;
        loop {
            let (random_num, sequence) = get_prng_number(state, *cycle_meta_data.last_roll())?;

            if let Some(delegate) = rolls_map.get(&random_num) {
                endorsers_slots.push(delegate.clone());
                break;
            } else {
                state = sequence;
            }
        }
    }
    Ok(endorsers_slots)
}

type RandomSeedState = Vec<u8>;
type TezosPRNGResult = Result<(i32, RandomSeedState), EndorsingRightsCalculationError>;

/// Initialize Tezos PRNG
///
/// # Arguments
///
/// * `state` - RandomSeedState, initially the random seed.
/// * `nonce_size` - Nonce_length from current protocol constants.
/// * `blocks_per_cycle` - Blocks_per_cycle from current protocol context constants
/// * `use_string_bytes` - String converted to bytes, i.e. endorsing rights use b"level endorsement:".
/// * `level` - block level
/// * `offset` - For baking priority, for endorsing slot
///
/// Return first random sequence state to use in [get_prng_number](`get_prng_number`)
#[inline]
pub fn init_prng(
    cycle_meta_data: &CycleData,
    constants: &ProtocolConstants,
    use_string_bytes: &[u8],
    cycle_position: i32,
    offset: i32,
) -> Result<RandomSeedState, EndorsingRightsCalculationError> {
    // a safe way to convert betwwen types is to use try_from
    let nonce_size = usize::from(constants.nonce_length);
    let state = cycle_meta_data.seed_bytes();
    let zero_bytes: Vec<u8> = vec![0; nonce_size];

    // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
    let rd = blake2b::digest_256(
        &[
            state,
            &zero_bytes,
            use_string_bytes,
            &cycle_position.to_be_bytes(),
        ]
        .concat(),
    )?;

    // take the 4 highest bytes and xor them with the priority/slot (offset)
    let higher = num_from_slice!(rd, 0, i32) ^ offset;

    // set the 4 highest bytes to the result of the xor operation
    let sequence = blake2b::digest_256(&[&higher.to_be_bytes(), &rd[4..]].concat())?;

    Ok(sequence)
}

/// Get pseudo random nuber using Tezos PRNG
///
/// # Arguments
///
/// * `state` - RandomSeedState, initially the random seed.
/// * `bound` - Last possible roll nuber that have meaning to be generated taken from [RightsContextData.last_roll](`RightsContextData.last_roll`).
///
/// Return pseudo random generated roll number and RandomSeedState for next roll generation if the roll provided is missing from the roll list
#[inline]
pub fn get_prng_number(state: RandomSeedState, bound: i32) -> TezosPRNGResult {
    if bound < 1 {
        return Err(EndorsingRightsCalculationError::BoundNotCorrect(bound));
    }
    let v: i32;
    // Note: this part aims to be similar
    // hash once again and take the 4 highest bytes and we got our random number
    let mut sequence = state;
    loop {
        let hashed = blake2b::digest_256(&sequence)?.to_vec();

        // computation for overflow check
        let drop_if_over = i32::max_value() - (i32::max_value() % bound);

        // 4 highest bytes
        let r = num_from_slice!(hashed, 0, i32).abs();

        // potentional overflow, keep the state of the generator and do one more iteration
        sequence = hashed;
        if r >= drop_if_over {
            continue;
        // use the remainder(mod) operation to get a number from a desired interval
        } else {
            v = r % bound;
            break;
        };
    }
    Ok((v, sequence))
}
