// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use anyhow::bail;
use crypto::hash::ProtocolHash;
use getset::Getters;
use thiserror::Error;

use crypto::{
    blake2b::{self, Blake2bError},
    crypto_box::PublicKeyError,
};
use storage::{
    cycle_eras_storage::{CycleEra, CycleErasData},
    cycle_storage::CycleData,
    CycleErasStorage,
};
use storage::{num_from_slice, BlockHeaderWithHash, CycleMetaStorage};

use crate::merge_slices;
use crate::server::RpcServiceEnvironment;
use crate::services::protocol::ContextProtocolParam;

use super::ProtocolConstants;

/// Context constants used in baking and endorsing rights
#[allow(dead_code)]
#[derive(Debug, Clone, Getters)]
pub struct RightsConstants {
    #[get = "pub(crate)"]
    blocks_per_cycle: i32,
    #[get = "pub(crate)"]
    preserved_cycles: u8,
    #[get = "pub(crate)"]
    nonce_length: u8,
    #[get = "pub(crate)"]
    time_between_blocks: Vec<i64>,
    #[get = "pub(crate)"]
    endorsers_per_block: u16,
    #[get = "pub(crate)"]
    minimal_block_delay: i64,

    // include the cycle eras in the constants
    #[get = "pub(crate)"]
    cycle_eras: CycleErasData,
}

#[derive(Debug, Error)]
pub enum RightsConstantError {
    #[error("The value is illegal, key: {key}")]
    WrongValue { key: &'static str },
}

impl RightsConstants {
    /// Get all context constants which are used in endorsing and baking rights generation
    ///
    /// # Arguments
    ///
    /// * `context_proto_param`
    #[inline]
    pub(crate) fn parse_rights_constants(
        context_proto_param: &ContextProtocolParam,
        env: &RpcServiceEnvironment,
    ) -> Result<Self, anyhow::Error> {
        let protocol_constants: ProtocolConstants =
            serde_json::from_str(&context_proto_param.constants_data)?;

        let cycle_eras = if let Some(eras) = CycleErasStorage::new(env.persistent_storage()).get(
            &ProtocolHash::from_base58_check(&context_proto_param.protocol_hash.protocol_hash())?,
        )? {
            eras
        } else {
            bail!("No cycle eras found!!")
        };

        Ok(Self {
            blocks_per_cycle: protocol_constants.blocks_per_cycle,
            preserved_cycles: protocol_constants.preserved_cycles,
            nonce_length: protocol_constants.nonce_length,
            time_between_blocks: protocol_constants.time_between_blocks,
            endorsers_per_block: protocol_constants.endorsers_per_block,
            minimal_block_delay: protocol_constants.minimal_block_delay,
            cycle_eras,
        })
    }
}

pub(crate) fn get_cycle_data(
    parameters: RightsParams,
    block_cycle: i32,
    cycle_meta_storage: &CycleMetaStorage,
) -> Result<CycleData, anyhow::Error> {
    // prepare cycle for which rollers are selected
    let requested_cycle = if let Some((cycle, _)) = *parameters.requested_cycle() {
        cycle
    } else {
        block_cycle
    };

    if let Some(cycle_meta_data) = cycle_meta_storage.get(&requested_cycle)? {
        Ok(cycle_meta_data)
    } else {
        bail!(
            "No cycle data found for requested_cycle: {}",
            requested_cycle
        )
    }
}

/// Set of parameters used to complete baking and endorsing rights
#[derive(Debug, Clone, Getters)]
pub struct RightsParams {
    /// Level (height) of block. Parsed from url path parameter 'block_id'.
    #[get = "pub(crate)"]
    block_level: i32,

    /// Header timestamp of block. Parsed from url path parameter 'block_id'.
    #[get = "pub(crate)"]
    block_timestamp: i64,

    /// Contract id to filter output by delegate. Url query parameter 'delegate'.
    #[get = "pub(crate)"]
    requested_delegate: Option<String>,

    // Cycle for whitch all rights will be listed with the blocks_per_cycle for that specific cycle Url query parameter 'cycle'./
    #[get = "pub(crate)"]
    requested_cycle: Option<(i32, CycleEra)>,

    /// Level (height) of block for whitch all rights will be listed. Url query parameter 'level'.
    #[get = "pub(crate)"]
    requested_level: i32,

    /// Level to be displayed in output.
    #[get = "pub(crate)"]
    display_level: i32,

    /// Level for estimated_time computation. Endorsing rights only.
    #[get = "pub(crate)"]
    timestamp_level: i32,

    /// Max priority to which baking rights are listed. Url query parameter 'max_priority'.
    #[get = "pub(crate)"]
    max_priority: i32,

    /// Indicate that baking rights for maximum priority should be listed. Url query parameter 'all'.
    #[get = "pub(crate)"]
    has_all: bool,

    #[get = "pub(crate)"]
    rights_metadata: RightsMetadata,
}

impl RightsParams {
    /// Simple constructor to create RightsParams
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_level: i32,
        block_timestamp: i64,
        requested_delegate: Option<String>,
        requested_cycle: Option<(i32, CycleEra)>,
        requested_level: i32,
        display_level: i32,
        timestamp_level: i32,
        max_priority: i32,
        has_all: bool,
        rights_metadata: RightsMetadata,
    ) -> Self {
        Self {
            block_level,
            block_timestamp,
            requested_delegate,
            requested_cycle,
            requested_level,
            display_level,
            timestamp_level,
            max_priority,
            has_all,
            rights_metadata,
        }
    }

    /// Prepare baking and endorsing rights parameters
    ///
    /// # Arguments
    ///
    /// * `param_level` - Url query parameter 'level'.
    /// * `param_delegate` - Url query parameter 'delegate'.
    /// * `param_cycle` - Url query parameter 'cycle'.
    /// * `param_max_priority` - Url query parameter 'max_priority'.
    /// * `param_has_all` - Url query parameter 'all'.
    /// * `block_level` - Block level from block_id.
    /// * `rights_constants` - Context constants used in baking and endorsing rights.
    /// * `context_list` - Context list handler.
    /// * `persistent_storage` - Persistent storage handler.
    /// * `is_baking_rights` - flag to identify if are parsed baking or endorsing rights
    ///
    /// Return RightsParams
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn parse_rights_parameters(
        param_level: Option<&str>,
        param_delegate: Option<&str>,
        param_cycle: Option<&str>,
        param_max_priority: Option<&str>,
        param_has_all: bool,
        rights_constants: &RightsConstants,
        block_header: &BlockHeaderWithHash,
        is_baking_rights: bool,
    ) -> Result<Self, anyhow::Error> {
        let block_level = block_header.header.level();
        let preserved_cycles = *rights_constants.preserved_cycles();

        // display_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        let mut display_level: i32 = block_level;
        // endorsing rights only: timestamp_level is the base level for timestamp computation is taken from last known block (block_id) timestamp + time_between_blocks[0]
        let mut timestamp_level: i32 = block_level;
        // Check the param_level, if there is a level specified validate it, if no set it to the block_level
        // requested_level is used to get data from context list
        let requested_level: i32 = match param_level {
            Some(level) => {
                let level = level.parse()?;
                // check the bounds for the requested level (if it is in the previous/next preserved cycles)
                // display level is always same as level requested
                display_level = level;
                // endorsing rights: to compute timestamp for level parameter there need to be taken timestamp of previous block
                timestamp_level = level - 1;

                level
            }
            // here is the main difference between endorsing and baking rights which data are loaded from context list based on level
            None => {
                if is_baking_rights {
                    display_level = block_level + 1;
                    block_level + 1
                } else {
                    block_level
                }
            }
        };

        // cycle era for the block entered as block_id (/chains/main/blocks/:block_id/...)
        let block_cycle_era = get_cycle_era_from_level(block_level, &rights_constants.cycle_eras)?;

        // cycle era for the requested_level
        let requested_cycle_era =
            get_cycle_era_from_level(requested_level, &rights_constants.cycle_eras)?;

        let rights_metadata = RightsMetadata::calculate(&requested_cycle_era, requested_level)?;

        Self::validate_cycle(
            cycle_from_level(requested_level, &requested_cycle_era)?,
            cycle_from_level(block_level, &block_cycle_era)?,
            preserved_cycles,
        )?;

        // validate requested cycle
        let requested_cycle = match param_cycle {
            Some(val) => {
                let parsed_cycle = Self::validate_cycle(
                    val.parse()?,
                    cycle_from_level(block_level, &block_cycle_era)?,
                    preserved_cycles,
                )?;

                let cycle_era =
                    get_cycle_era_from_cycle(parsed_cycle, &rights_constants.cycle_eras)?;

                Some((parsed_cycle, cycle_era))
            }
            None => None,
        };

        // set max_priority from param value or default
        let max_priority = match param_max_priority {
            Some(val) => val.parse()?,
            None => {
                if requested_cycle.is_some() {
                    8
                } else {
                    64
                }
            }
        };

        Ok(Self::new(
            block_level,
            block_header.header.timestamp(),
            param_delegate.map(String::from),
            requested_cycle,
            requested_level,
            display_level,   // endorsing rights only
            timestamp_level, // endorsing rights only
            max_priority,
            param_has_all,
            rights_metadata,
        ))
    }

    /// Compute estimated time for endorsing rights
    ///
    /// # Arguments
    ///
    /// * `constants` - Context constants used in baking and endorsing rights.
    /// * `level` - Optional level for which is timestamp computed.
    ///
    /// If level is not provided [timestamp_level](`RightsParams.timestamp_level`) is used fro computation
    #[inline]
    pub fn get_estimated_time(
        &self,
        constants: &RightsConstants,
        level: Option<i32>,
    ) -> Option<i64> {
        // if is cycle then level is provided as parameter else use prepared timestamp_level
        let timestamp_level = level.unwrap_or(self.timestamp_level);

        //check if estimated time is computed and convert from raw epoch time to rfc3339 format
        if self.block_level <= timestamp_level {
            let est_timestamp = ((timestamp_level - self.block_level).abs() as i64
                * constants.minimal_block_delay())
                + self.block_timestamp;
            Some(est_timestamp)
        } else {
            None
        }
    }

    /// Validate if cycle requested as url query parameter (cycle or level) is available in context list by checking preserved_cycles constant
    #[inline]
    fn validate_cycle(
        requested_cycle: i32,
        current_cycle: i32,
        preserved_cycles: u8,
    ) -> Result<i32, anyhow::Error> {
        if (requested_cycle - current_cycle).abs() <= preserved_cycles.into() {
            Ok(requested_cycle)
        } else {
            // TODO: proper json response is needed for this
            // Octez is:
            //    [{ "kind": "permanent", "id": "proto.008-PtEdo2Zk.seed.unknown_seed",
            //        "oldest": 330, "requested": 200, "latest": 340 }]
            bail!("Requested cycle out of bounds") //TODO: prepare cycle error
        }
    }
}

/// Struct used in endorsing rights to map endorsers to endorsement slots before they can be ordered and completed
#[derive(Debug, Clone, Getters)]
pub struct EndorserSlots {
    /// endorser contract id in form:tz1.../tz2.../KT1...
    #[get = "pub(crate)"]
    contract_id: String,

    /// Orderer vector of endorsement slots
    #[get = "pub(crate)"]
    slots: Vec<u16>,
}

impl EndorserSlots {
    /// Simple constructor that return EndorserSlots
    pub fn new(contract_id: String, slots: Vec<u16>) -> Self {
        Self { contract_id, slots }
    }

    /// Push endorsing slot to slots
    pub fn push_to_slot(&mut self, slot: u16) {
        self.slots.push(slot);
    }
}

/// Enum defining Tezos PRNG possible error
#[derive(Debug, Error)]
pub enum TezosPRNGError {
    #[error("Value of bound(last_roll) not correct: {bound} bytes")]
    BoundNotCorrect { bound: i32 },
    #[error("Public key error: {0}")]
    PublicKeyError(PublicKeyError),
}

impl From<PublicKeyError> for TezosPRNGError {
    fn from(source: PublicKeyError) -> Self {
        TezosPRNGError::PublicKeyError(source)
    }
}

impl From<Blake2bError> for TezosPRNGError {
    fn from(source: Blake2bError) -> Self {
        TezosPRNGError::PublicKeyError(source.into())
    }
}

type RandomSeedState = Vec<u8>;
pub type TezosPRNGResult = Result<(i32, RandomSeedState), TezosPRNGError>;

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
    constants: &RightsConstants,
    use_string_bytes: &[u8],
    cycle_position: i32,
    offset: i32,
) -> Result<RandomSeedState, anyhow::Error> {
    // a safe way to convert betwwen types is to use try_from
    let nonce_size = usize::try_from(*constants.nonce_length())?;
    let state = cycle_meta_data.seed_bytes();
    let zero_bytes: Vec<u8> = vec![0; nonce_size];

    // take the state (initially the random seed), zero bytes, the use string and the blocks position in the cycle as bytes, merge them together and hash the result
    let rd = blake2b::digest_256(&merge_slices!(
        state,
        &zero_bytes,
        use_string_bytes,
        &cycle_position.to_be_bytes()
    ))?
    .to_vec();

    // take the 4 highest bytes and xor them with the priority/slot (offset)
    let higher = num_from_slice!(rd, 0, i32) ^ offset;

    // set the 4 highest bytes to the result of the xor operation
    let sequence = blake2b::digest_256(&merge_slices!(&higher.to_be_bytes(), &rd[4..]))?;

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
        return Err(TezosPRNGError::BoundNotCorrect { bound });
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

#[derive(Debug, Clone, Getters)]
pub struct RightsMetadata {
    #[get = "pub(crate)"]
    block_cycle: i32,

    #[get = "pub(crate)"]
    block_cycle_position: i32,
}

impl RightsMetadata {
    pub fn calculate(era: &CycleEra, requested_level: i32) -> Result<Self, RightsConstantError> {
        Ok(Self {
            block_cycle: cycle_from_level(requested_level, era)?,
            block_cycle_position: level_position(requested_level, era)?,
        })
    }
}

/// Return cycle in which is given level
///
/// # Arguments
///
/// * `level` - level to specify cycle for
/// * `blocks_per_cycle` - context constant
///
/// Level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1),
/// hence the blocks_per_cycle - 1 for last cycle block.
pub fn cycle_from_level(level: i32, era: &CycleEra) -> Result<i32, RightsConstantError> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if *era.blocks_per_cycle() > 0 {
        Ok((level - *era.first_level()) / *era.blocks_per_cycle() + *era.first_cycle())
    } else {
        Err(RightsConstantError::WrongValue {
            key: "blocks_per_cycle",
        })
    }
}

/// Return the position of the block in its cycle
///
/// # Arguments
///
/// * `level` - level to specify cycle for
/// * `blocks_per_cycle` - context constant
///
/// Level 0 (genesis block) is not part of any cycle (cycle 0 starts at level 1),
/// hence the blocks_per_cycle - 1 for last cycle block.
pub fn level_position(level: i32, era: &CycleEra) -> Result<i32, RightsConstantError> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if *era.blocks_per_cycle() <= 0 {
        return Err(RightsConstantError::WrongValue {
            key: "blocks_per_cycle",
        });
    }
    let cycle_position = (level - era.first_level()) % era.blocks_per_cycle();
    if cycle_position < 0 {
        //for last block
        Ok(era.blocks_per_cycle() - 1)
    } else {
        Ok(cycle_position)
    }
}

fn get_cycle_era_from_level(level: i32, eras: &[CycleEra]) -> Result<CycleEra, anyhow::Error> {
    for era in eras {
        if *era.first_level() > level {
            continue;
        } else {
            return Ok(era.clone());
        }
    }

    bail!("No matching cycle Era found!")
}

fn get_cycle_era_from_cycle(cycle: i32, eras: &[CycleEra]) -> Result<CycleEra, anyhow::Error> {
    for era in eras {
        if *era.first_cycle() > cycle {
            continue;
        } else {
            return Ok(era.clone());
        }
    }

    bail!("No matching cycle Era found!")
}
