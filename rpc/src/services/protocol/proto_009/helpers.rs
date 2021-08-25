// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;
use std::sync::Arc;

use failure::{bail, Fail};
use getset::Getters;

use crypto::hash::ChainId;
use crypto::{
    blake2b::{self, Blake2bError},
    crypto_box::PublicKeyError,
};
use storage::cycle_storage::CycleData;
use storage::{num_from_slice, BlockHeaderWithHash, CycleMetaStorage};

use crate::helpers::{parse_block_hash, BlockMetadata};
use crate::merge_slices;
use crate::server::RpcServiceEnvironment;
use crate::services::protocol::{
    parse_block_metadata_level, ContextProtocolParam, MetadataParsingError,
};

/// Context constants used in baking and endorsing rights
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
}

#[derive(Debug, Fail)]
pub enum RightsConstantError {
    #[fail(display = "The value is illegal, key: {}", key)]
    WrongValue { key: &'static str },
    #[fail(display = "Key cannot be parsed, key: {}", key)]
    Parsing { key: &'static str },
    #[fail(display = "Key cannot be found in constants, key: {}", key)]
    KeyNotFound { key: &'static str },
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
    ) -> Result<Self, failure::Error> {
        // let constatns_deserialized = context_proto_param.constants_data;

        // TODO: fix unwraps
        if let Some(constatns_deserialized) =
            serde_json::from_str::<serde_json::Value>(&context_proto_param.constants_data)?
                .as_object()
        {
            let blocks_per_cycle = get_constant_i64(constatns_deserialized, "blocks_per_cycle")?;
            let preserved_cycles = get_constant_i64(constatns_deserialized, "preserved_cycles")?;
            let nonce_length = get_constant_i64(constatns_deserialized, "nonce_length")?;
            let time_between_blocks = get_time_between_blocks(constatns_deserialized)?;
            let endorsers_per_block =
                get_constant_i64(constatns_deserialized, "endorsers_per_block")?;

            Ok(Self {
                blocks_per_cycle: blocks_per_cycle as i32,
                preserved_cycles: preserved_cycles as u8,
                nonce_length: nonce_length as u8,
                time_between_blocks,
                endorsers_per_block: endorsers_per_block as u16,
            })
        } else {
            bail!("Constants not an object")
        }
    }
}

fn get_constant_i64(
    constants: &serde_json::Map<String, serde_json::Value>,
    key: &'static str,
) -> Result<i64, RightsConstantError> {
    if let Some(value) = constants.get(key) {
        if let Some(i64_value) = value.as_i64() {
            Ok(i64_value)
        } else {
            if let Some(str_num) = value.as_str() {
                return str_num
                    .parse()
                    .map_err(|_| RightsConstantError::Parsing { key });
            }
            Err(RightsConstantError::Parsing { key })
        }
    } else {
        Err(RightsConstantError::KeyNotFound { key })
    }
}

fn get_time_between_blocks(
    constants: &serde_json::Map<String, serde_json::Value>,
) -> Result<Vec<i64>, RightsConstantError> {
    let mut ret: Vec<i64> = Vec::new();
    if let Some(value) = constants.get("time_between_blocks") {
        if let Some(array) = value.as_array() {
            for e in array {
                if let Some(str_element) = e.as_str() {
                    let parsed =
                        str_element
                            .parse::<i64>()
                            .map_err(|_| RightsConstantError::Parsing {
                                key: "time_between_blocks",
                            })?;
                    ret.push(parsed);
                } else {
                    return Err(RightsConstantError::Parsing {
                        key: "time_between_blocks",
                    });
                }
            }
            Ok(ret)
        } else {
            Err(RightsConstantError::Parsing {
                key: "time_between_blocks",
            })
        }
    } else {
        Err(RightsConstantError::KeyNotFound {
            key: "time_between_blocks",
        })
    }
}

pub(crate) fn get_cycle_data(
    parameters: RightsParams,
    block_cycle: i32,
    cycle_meta_storage: &CycleMetaStorage,
) -> Result<CycleData, failure::Error> {
    // prepare cycle for which rollers are selected
    let requested_cycle = if let Some(cycle) = *parameters.requested_cycle() {
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

    /// Cycle for whitch all rights will be listed. Url query parameter 'cycle'.
    #[get = "pub(crate)"]
    requested_cycle: Option<i32>,

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
        requested_cycle: Option<i32>,
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
        chain_id: &ChainId,
        env: &RpcServiceEnvironment,
        is_baking_rights: bool,
    ) -> Result<Self, failure::Error> {
        let block_level: i32 = block_header.header.level();
        let preserved_cycles = *rights_constants.preserved_cycles();
        let blocks_per_cycle = *rights_constants.blocks_per_cycle();

        // this is the cycle of block_id level
        // let current_cycle = cycle_from_level(block_level, blocks_per_cycle)?;

        // display_level is here because of corner case where all levels < 1 are computed as level 1 but oputputed as they are
        let mut display_level: i32 = block_level;
        // endorsing rights only: timestamp_level is the base level for timestamp computation is taken from last known block (block_id) timestamp + time_between_blocks[0]
        let mut timestamp_level: i32 = block_level;
        // Check the param_level, if there is a level specified validate it, if no set it to the block_level
        // requested_level is used to get data from context list
        let requested_level: i32 = match param_level {
            Some(level) => {
                let level = level.parse()?;
                // display level is always same as level requested
                display_level = level;
                // endorsing rights: to compute timestamp for level parameter there need to be taken timestamp of previous block
                timestamp_level = level - 1;
                // there can be requested also negative level, this is not an error but it is handled same as level 1 would be requested
                // In Ocaml Tezos node there can be negative level provided as query parameter, it is not handled as error but it will instead return endorsing/baking rights for level 1
                // for all reuqested negative levels use level 1
                if level < 1 {
                    1
                } else {
                    level
                }
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

        let block_metadata = match crate::services::base_services::get_block_metadata(
            chain_id,
            &parse_block_hash(chain_id, &requested_level.to_string(), env)?,
            env,
        )
        .await
        {
            Ok(metadata) => Some(metadata),
            Err(_) => None,
        };

        let rights_metadata = if let Some(metadata) = block_metadata {
            RightsMetadata::extract_from_block_metadata(metadata)?
        } else if param_level.is_some() {
            RightsMetadata::calculate(blocks_per_cycle, requested_level)?
        } else {
            // this should never happen. When block_metadata is not available it means we requested a future level (thus level is always Some())
            bail!("No level parameter in url")
        };

        Self::validate_cycle(
            cycle_from_level(requested_level, blocks_per_cycle)?,
            rights_metadata.block_cycle,
            preserved_cycles,
        )?;

        // validate requested cycle
        let requested_cycle = match param_cycle {
            Some(val) => Some(Self::validate_cycle(
                val.parse()?,
                rights_metadata.block_cycle,
                preserved_cycles,
            )?),
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
                * constants.time_between_blocks()[0])
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
    ) -> Result<i32, failure::Error> {
        if (requested_cycle - current_cycle).abs() <= (preserved_cycles as i32) {
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
#[derive(Debug, Fail)]
pub enum TezosPRNGError {
    #[fail(display = "Value of bound(last_roll) not correct: {} bytes", bound)]
    BoundNotCorrect { bound: i32 },
    #[fail(display = "Public key error: {}", _0)]
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
) -> Result<RandomSeedState, failure::Error> {
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
    pub fn extract_from_block_metadata(
        block_metadata: Arc<BlockMetadata>,
    ) -> Result<Self, MetadataParsingError> {
        let (block_cycle, block_cycle_position) =
            match parse_block_metadata_level(&block_metadata, "level_info") {
                Ok(cycle_data) => cycle_data,
                Err(MetadataParsingError::KeyNotFoundError { key: _ }) => {
                    // No level info found, trying the older scheme
                    parse_block_metadata_level(&block_metadata, "level")?
                }
                Err(e) => return Err(e),
            };
        Ok(Self {
            block_cycle,
            block_cycle_position,
        })
    }

    pub fn calculate(
        blocks_per_cycle: i32,
        requested_level: i32,
    ) -> Result<Self, RightsConstantError> {
        Ok(Self {
            // (cycle_from_level(requested_level, blocks_per_cycle)?, level_position(requested_level, blocks_per_cycle)?)
            block_cycle: cycle_from_level(requested_level, blocks_per_cycle)?,
            block_cycle_position: level_position(requested_level, blocks_per_cycle)?,
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
pub fn cycle_from_level(level: i32, blocks_per_cycle: i32) -> Result<i32, RightsConstantError> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle > 0 {
        Ok((level - 1) / blocks_per_cycle)
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
pub fn level_position(level: i32, blocks_per_cycle: i32) -> Result<i32, RightsConstantError> {
    // check if blocks_per_cycle is not 0 to prevent panic
    if blocks_per_cycle <= 0 {
        return Err(RightsConstantError::WrongValue {
            key: "blocks_per_cycle",
        });
    }
    let cycle_position = (level % blocks_per_cycle) - 1;
    if cycle_position < 0 {
        //for last block
        Ok(blocks_per_cycle - 1)
    } else {
        Ok(cycle_position)
    }
}
