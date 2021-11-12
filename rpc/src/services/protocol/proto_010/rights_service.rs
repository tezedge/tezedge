// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

use anyhow::format_err;
use itertools::Itertools;

use crate::server::RpcServiceEnvironment;
use crate::services::dev_services::contract_id_to_contract_address_for_index;
use tezos_messages::base::rpc_support::{RpcJsonMap, ToRpcJsonMap};
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;
use tezos_messages::protocol::proto_010::rights::{BakingRights, EndorsingRight};

use storage::cycle_storage::CycleData;
use storage::CycleMetaStorage;

use crate::services::protocol::proto_010::helpers::{
    get_cycle_data, get_prng_number, init_prng, level_position, EndorserSlots, RightsConstants,
    RightsMetadata, RightsParams,
};
use crate::services::protocol::ContextProtocolParam;

const ONE_MINUTE: i64 = 60;

/// Return generated baking rights.
///
/// # Arguments
///
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `max_priority` - Url query parameter 'max_priority'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate baking rights and then use Tezos PRNG to generate them.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn check_and_get_baking_rights(
    context_proto_params: ContextProtocolParam,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    max_priority: Option<&str>,
    has_all: bool,
    cycle_meta_storage: &CycleMetaStorage,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, anyhow::Error> {
    let constants: RightsConstants =
        RightsConstants::parse_rights_constants(&context_proto_params, env)?;

    // parse metadata if there is Some(metadata), the block is known and in storage
    // let rights_metadata = RightsMetadata::create_rights_metadata(block_metadata, *constants.blocks_per_cycle(), requested_level)?;

    let params: RightsParams = RightsParams::parse_rights_parameters(
        level,
        delegate,
        cycle,
        max_priority,
        has_all,
        &constants,
        &context_proto_params.block_header,
        true,
    )
    .await?;

    let cycle_meta_data = get_cycle_data(
        params.clone(),
        *params.rights_metadata().block_cycle(),
        cycle_meta_storage,
    )?;

    get_baking_rights(
        &cycle_meta_data,
        &params,
        &constants,
        params.rights_metadata(),
    )
}

/// Use prepared data to generate baking rights
///
/// # Arguments
///
/// * `cycle_meta_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [RightsConstants](RightsConstants::parse_rights_constants).
#[inline]
pub(crate) fn get_baking_rights(
    cycle_meta_data: &CycleData,
    parameters: &RightsParams,
    constants: &RightsConstants,
    rights_metadata: &RightsMetadata,
) -> Result<Option<Vec<RpcJsonMap>>, anyhow::Error> {
    let mut baking_rights = Vec::<BakingRights>::new();

    let timestamp = parameters.block_timestamp();
    let cycle_position = *rights_metadata.block_cycle_position();

    // build a reverse map of rols so we have access in O(1)
    let mut rolls_map: HashMap<i32, String> = HashMap::new();

    for (delegate, rolls) in cycle_meta_data.rolls_data() {
        for roll in rolls {
            rolls_map.insert(
                *roll,
                SignaturePublicKeyHash::from_tagged_bytes(delegate.to_vec())?
                    .to_string_representation(),
            );
        }
    }

    // iterate through the whole cycle if necessery
    if let Some((cycle, cycle_era)) = parameters.requested_cycle() {
        let first_level_in_era = *cycle_era.first_level();
        let first_cycle_in_era = *cycle_era.first_cycle();
        let blocks_per_cycle_in_era = *cycle_era.blocks_per_cycle();
        let first_block_level =
            blocks_per_cycle_in_era * (cycle - first_cycle_in_era) + first_level_in_era;
        let last_block_level = first_block_level + blocks_per_cycle_in_era;

        for level in first_block_level..last_block_level {
            let cycle_position = level_position(level, cycle_era)?;

            // assign rolls goes here
            baking_rights_assign_rolls(
                parameters,
                constants,
                cycle_meta_data,
                &rolls_map,
                level,
                cycle_position,
                *timestamp,
                true,
                &mut baking_rights,
            )?;

            // baking_rights = merge_slices!(&baking_rights, &level_baking_rights);
        }
    } else {
        let level = *parameters.requested_level();
        // assign rolls goes here
        baking_rights_assign_rolls(
            parameters,
            constants,
            cycle_meta_data,
            &rolls_map,
            level,
            cycle_position,
            *timestamp,
            false,
            &mut baking_rights,
        )?;
    }

    // if there is some delegate specified, retrive his priorities
    if let Some(delegate) = parameters.requested_delegate() {
        let delegate = &SignaturePublicKeyHash::from_b58_hash(delegate)?;
        Ok(Some(
            baking_rights
                .into_iter()
                .filter(|val| val.delegate.eq(delegate))
                .map(|val| val.as_map())
                .collect::<Vec<RpcJsonMap>>(),
        ))
    } else {
        Ok(Some(
            baking_rights
                .into_iter()
                .map(|val| val.as_map())
                .collect::<Vec<RpcJsonMap>>(),
        ))
    }
}

/// Use prepared data to generate baking rights
///
/// # Arguments
///
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [RightsConstants](RightsConstants::parse_rights_constants).
/// * `cycle_meta_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `rolls_map` - Inverted mapping of the rolls, where each delegate is mapped to the roll number.
/// * `level` - Level to feed Tezos PRNG.
/// * `block_timestamp` - Estimated time of baking, is set to None if in past relative to block_id.
///
/// Baking priorities are are assigned to Roles, the default behavior is to include only the top priority for the delegate
#[inline]
#[allow(clippy::too_many_arguments)]
fn baking_rights_assign_rolls(
    parameters: &RightsParams,
    constants: &RightsConstants,
    cycle_meta_data: &CycleData,
    rolls_map: &HashMap<i32, String>,
    level: i32,
    cycle_position: i32,
    block_timestamp: i64,
    is_cycle: bool,
    baking_rights: &mut Vec<BakingRights>,
) -> Result<(), anyhow::Error> {
    const BAKING_USE_STRING: &[u8] = b"level baking:";

    // hashset is defined to keep track of the delegates with priorities already assigned
    let mut assigned = HashSet::new();

    let minimal_block_delay = constants.minimal_block_delay();
    let max_priority = *parameters.max_priority();
    let has_all = parameters.has_all();
    let block_level = *parameters.block_level();
    let last_roll = *cycle_meta_data.last_roll();
    let display_level: i32 = *parameters.display_level();

    let block_level_diff: i64 = (level - block_level).abs().into();
    let seconds_to_add: i64 = block_level_diff * minimal_block_delay;

    let predecessor_timestamp = if block_level_diff > 1 {
        // predecessor is off by one minimal_block_delay
        block_timestamp + seconds_to_add - minimal_block_delay
    } else {
        // Note: level here is the requested_level, that is always incremented by 1 when no specific ?level= is included
        // so it's predecessor is allways the current block
        block_timestamp
    };

    for priority in 0..=max_priority {
        // draw the rolls for the requested parameters
        let delegate_to_assign;
        // TODO: priority can overflow in the ocaml code, do a priority % i32::max_value()
        let mut state = init_prng(
            cycle_meta_data,
            constants,
            BAKING_USE_STRING,
            cycle_position,
            priority,
        )?;

        loop {
            let (random_num, sequence) = get_prng_number(state, last_roll)?;

            if let Some(d) = rolls_map.get(&random_num) {
                delegate_to_assign = d;
                break;
            } else {
                state = sequence;
            }
        }

        // if the delegate was assgined and the the has_all flag is not set skip this priority
        if assigned.contains(&delegate_to_assign) && !has_all {
            continue;
        }

        // we omit the estimated_time field if the block on the requested level is already baked
        let priority_timestamp = if block_level < level {
            Some(minimal_time(constants, priority, predecessor_timestamp))
        } else {
            None
        };

        // used to handle corner cases, where the requested level < 1, not necessary when handling whole cycles
        let rights_level = if is_cycle { level } else { display_level };

        baking_rights.push(BakingRights::new(
            rights_level,
            SignaturePublicKeyHash::from_b58_hash(delegate_to_assign)?,
            priority as u16,
            priority_timestamp,
        ));
        assigned.insert(delegate_to_assign);
    }
    Ok(())
}

// https://gitlab.com/tezos/tezos/-/blob/685227895f48a2564a9de3e1e179e7cd741d52fb/src/proto_010_PtGRANAD/lib_protocol/baking.ml#L183-220
fn minimal_time(constants: &RightsConstants, priority: i32, block_timestamp: i64) -> i64 {
    if priority == 0 {
        let minimal_block_delay = *constants.minimal_block_delay();
        block_timestamp + minimal_block_delay
    } else {
        minimal_time_slowpath_case(constants, priority, block_timestamp)
    }
}

fn minimal_time_slowpath_case(
    constants: &RightsConstants,
    input_priority: i32,
    block_timestamp: i64,
) -> i64 {
    let time_between_blocks = constants.time_between_blocks().as_slice();
    let mut accumulated_timestamp = block_timestamp;
    let mut current_priority = 1 + input_priority;
    let mut durations = if time_between_blocks.is_empty() {
        // If there are no durations, a single duration of one minute is the default
        &[ONE_MINUTE]
    } else {
        time_between_blocks
    };

    loop {
        if current_priority == 0 {
            return accumulated_timestamp;
        } else {
            let time = durations[0];

            if durations.len() == 1 {
                return accumulated_timestamp + (current_priority as i64 * time);
            } else {
                accumulated_timestamp += time;
                current_priority -= 1;
                durations = &durations[1..];
            }
        }
    }
}

/// Return generated endorsing rights.
///
/// # Arguments
///
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate endorsing rights and then use Tezos PRNG to generate them.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn check_and_get_endorsing_rights(
    context_proto_params: ContextProtocolParam,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    has_all: bool,
    cycle_meta_storage: &CycleMetaStorage,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, anyhow::Error> {
    let constants: RightsConstants =
        RightsConstants::parse_rights_constants(&context_proto_params, env)?;

    let params: RightsParams = RightsParams::parse_rights_parameters(
        level,
        delegate,
        cycle,
        None,
        has_all,
        &constants,
        &context_proto_params.block_header,
        false,
    )
    .await?;

    let cycle_meta_data = get_cycle_data(
        params.clone(),
        *params.rights_metadata().block_cycle(),
        cycle_meta_storage,
    )?;

    get_endorsing_rights(&cycle_meta_data, &params, &constants)
}

/// Use prepared data to generate endosring rights
///
/// # Arguments
///
/// * `cycle_meta_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [RightsConstants](RightsConstants::parse_rights_constants).
fn get_endorsing_rights(
    cycle_meta_data: &CycleData,
    parameters: &RightsParams,
    constants: &RightsConstants,
) -> Result<Option<Vec<RpcJsonMap>>, anyhow::Error> {
    // define helper and output variables
    let mut endorsing_rights = Vec::<EndorsingRight>::new();

    // build a reverse map of rols so we have access in O(1)
    let mut rolls_map: HashMap<i32, String> = HashMap::new();

    for (delegate, rolls) in cycle_meta_data.rolls_data() {
        for roll in rolls {
            rolls_map.insert(
                *roll,
                SignaturePublicKeyHash::from_tagged_bytes(delegate.to_vec())?
                    .to_string_representation(),
            );
        }
    }

    // when query param cycle is specified then iterate over all cycle levels, else only given level
    if let Some((cycle, cycle_era)) = parameters.requested_cycle() {
        let first_level_in_era = *cycle_era.first_level();
        let first_cycle_in_era = *cycle_era.first_cycle();
        let blocks_per_cycle_in_era = *cycle_era.blocks_per_cycle();
        let first_block_level =
            blocks_per_cycle_in_era * (cycle - first_cycle_in_era) + first_level_in_era;
        let last_block_level = first_block_level + blocks_per_cycle_in_era;

        for level in first_block_level..last_block_level {
            // get estimated time first because is equal for all endorsers in given level
            // the base level for estimated time computation is level of previous block
            let estimated_time: Option<i64> =
                parameters.get_estimated_time(constants, Some(level - 1));
            let cycle_position = level_position(level, cycle_era)?;

            complete_endorsing_rights_for_level(
                cycle_meta_data,
                parameters,
                constants,
                &rolls_map,
                level,
                estimated_time,
                cycle_position,
                &mut endorsing_rights,
            )?;
        }
    } else {
        // use level prepared during parameter parsing to compute estimated time
        let estimated_time: Option<i64> = parameters.get_estimated_time(constants, None);

        complete_endorsing_rights_for_level(
            cycle_meta_data,
            parameters,
            constants,
            &rolls_map,
            *parameters.display_level(),
            estimated_time,
            *parameters.rights_metadata().block_cycle_position(),
            &mut endorsing_rights,
        )?;
    };

    Ok(Some(
        endorsing_rights
            .into_iter()
            .map(|val| val.as_map())
            .collect::<Vec<RpcJsonMap>>(),
    ))
}

/// Use prepared data to generate endosring rights
///
/// # Arguments
///
/// * `cycle_meta_data` - Data from context list used in baking and endorsing rights generation filled in [RightsContextData](RightsContextData::prepare_context_data_for_rights).
/// * `parameters` - Parameters created by [RightsParams](RightsParams::parse_rights_parameters).
/// * `constants` - Context constants used in baking and endorsing rights [RightsConstants](RightsConstants::parse_rights_constants).
/// * `rolls_map` - Inverted mapping of the rolls, where each delegate is mapped to the roll number.
/// * `level` - Level to feed Tezos PRNG.
/// * `display_level` - Level to be displayed in output.
/// * `estimated_time` - Estimated time of endorsement, is set to None if in past relative to block_id.
#[inline]
#[allow(clippy::too_many_arguments)]
fn complete_endorsing_rights_for_level(
    cycle_meta_data: &CycleData,
    parameters: &RightsParams,
    constants: &RightsConstants,
    rolls_map: &HashMap<i32, String>,
    display_level: i32,
    estimated_time: Option<i64>,
    cycle_position: i32,
    endorsing_rights: &mut Vec<EndorsingRight>,
) -> Result<(), anyhow::Error> {
    // endorsers_slots is needed to group all slots by delegate
    let endorsers_slots =
        get_endorsers_slots(constants, cycle_meta_data, rolls_map, cycle_position)?;

    // convert contract id to hash contract address hex byte string (needed for ordering)
    let mut endorers_slots_keys_for_order: HashMap<String, String> = HashMap::new();
    for key in endorsers_slots.keys() {
        endorers_slots_keys_for_order.insert(
            hex::encode(contract_id_to_contract_address_for_index(key.as_str())?),
            key.clone(),
        );
    }

    // order descending by delegate public key hash address hex byte string
    for delegate in endorers_slots_keys_for_order.keys().sorted().rev() {
        let delegate_key = endorers_slots_keys_for_order
            .get(delegate)
            .ok_or_else(|| format_err!("missing delegate key"))?;
        let delegate_data = endorsers_slots.get(delegate_key).ok_or_else(|| {
            format_err!("missing EndorserSlots for delegate_key: {:?}", delegate_key)
        })?;

        // prepare delegate contract id
        let delegate_contract_id = delegate_data.contract_id();

        // filter delegates
        if let Some(d) = parameters.requested_delegate() {
            if delegate_contract_id != d {
                continue;
            }
        }

        endorsing_rights.push(EndorsingRight::new(
            display_level,
            SignaturePublicKeyHash::from_b58_hash(delegate_contract_id)?,
            delegate_data.slots().clone(),
            estimated_time,
        ))
    }
    Ok(())
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
    constants: &RightsConstants,
    cycle_meta_data: &CycleData,
    rolls_map: &HashMap<i32, String>,
    cycle_position: i32,
) -> Result<HashMap<String, EndorserSlots>, anyhow::Error> {
    // special byte string used in Tezos PRNG
    const ENDORSEMENT_USE_STRING: &[u8] = b"level endorsement:";
    // prepare helper variable
    let mut endorsers_slots: HashMap<String, EndorserSlots> = HashMap::new();

    for endorser_slot in 0..*constants.endorsers_per_block() {
        // generate PRNG per endorsement slot and take delegates by roll number from context_rolls
        // if roll number is not found then reroll with new state till roll nuber is found in context_rolls
        let mut state = init_prng(
            cycle_meta_data,
            constants,
            ENDORSEMENT_USE_STRING,
            cycle_position,
            endorser_slot.try_into()?,
        )?;
        loop {
            let (random_num, sequence) = get_prng_number(state, *cycle_meta_data.last_roll())?;

            if let Some(delegate) = rolls_map.get(&random_num) {
                // collect all slots for each delegate
                let endorsers_slots_entry = endorsers_slots
                    .entry(delegate.clone())
                    .or_insert_with(|| EndorserSlots::new(delegate.clone(), Vec::new()));
                endorsers_slots_entry.push_to_slot(endorser_slot as u16);
                break;
            } else {
                state = sequence;
            }
        }
    }
    Ok(endorsers_slots)
}
