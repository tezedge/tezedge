use std::{
    collections::{BTreeMap, HashSet},
    str::FromStr,
};

use crate::{
    helpers::{parse_block_hash, parse_chain_id, BlockMetadata, BlockOperations, MAIN_CHAIN_ID},
    RpcServiceEnvironment,
};
use anyhow::bail;
use crypto::hash::{BlockHash, ChainId, ProtocolHash};
use num::{bigint::Sign, BigInt, BigRational};
use serde::{Deserialize, Serialize};
use storage::{
    cycle_eras_storage::CycleEra,
    reward_storage::{CycleRewardsInt, RewardStorage},
    BlockMetaStorage, BlockMetaStorageReader, BlockStorage, ConstantsStorage, CycleErasStorage,
};
use storage::{
    BlockAdditionalData, BlockHeaderWithHash, BlockJsonData, BlockStorageReader, OperationsStorage,
    OperationsStorageReader,
};
use tezos_api::ffi::{ApplyBlockRequest, RpcMethod, RpcRequest};
use tezos_messages::{
    p2p::encoding::{operation::Operation, operations_for_blocks::OperationsForBlocksMessage},
    protocol::{SupportedProtocol, UnsupportedProtocolError},
};
use tezos_protocol_ipc_client::ProtocolRunnerConnection;

use super::{base_services::get_additional_data_or_fail, protocol};

pub struct CycleRewardsFilter {
    pub delegate: Option<String>,
    pub commission: Option<i32>,
    pub exclude_accusation_rewards: bool,
}

impl CycleRewardsFilter {
    pub fn new(
        delegate: Option<String>,
        commission: Option<i32>,
        exclude_accusation_rewards: bool,
    ) -> Self {
        Self {
            delegate,
            commission,
            exclude_accusation_rewards,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BalanceUpdateKind {
    Contract(ContractKind),
    Accumulator,
    Freezer(FreezerKind),
    Minted,
    Burned,
    Commitment,
    Unknown,
}

impl Default for BalanceUpdateKind {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ContractKind {
    contract: String,
    change: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FreezerKind {
    delegate: String,
    change: String,
    category: BalanceUpdateCategory,
}

// TODO: include legacy stuff?
// Note: We need to rename the variants because of "tezos case" variants.....
#[derive(Clone, Debug, Deserialize)]
pub enum BalanceUpdateCategory {
    #[serde(rename = "block fees")]
    BlockFees,
    #[serde(rename = "deposits")]
    Deposits,
    #[serde(rename = "nonce revelation rewards")]
    NonceRevelationRewards,
    #[serde(rename = "double signing evidence rewards")]
    DoubleSigningEvidenceRewards,
    #[serde(rename = "endorsing rewards")]
    EndorsingRewards,
    #[serde(rename = "baking rewards")]
    BakingRewards,
    #[serde(rename = "baking bonuses")]
    BakingBonuses,
    #[serde(rename = "storage fees")]
    StorageFees,
    #[serde(rename = "punishment")]
    Punishment,
    #[serde(rename = "lost endorsing rewards")]
    LostEndorsingRewards,
    #[serde(rename = "subsidy")]
    Subsidy,
    #[serde(rename = "burned")]
    Burned,
    #[serde(rename = "commitment")]
    Commitment,
    #[serde(rename = "bootstrap")]
    Bootstrap,
    #[serde(rename = "invoice")]
    Invoice,
    #[serde(rename = "minted")]
    Minted,
    #[serde(rename = "rewards")]
    LegacyFrozenRewards,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BalanceUpdateOrigin {
    Block,
    Migration,
    Subsidy,
    Simulation,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DelegateInfo {
    staking_balance: String,
    delegated_contracts: Vec<String>,
}

impl DelegateInfo {
    async fn try_get_from_rpc(
        delegate: String,
        snapshot_info: &SnapshotCycleInfo,
        env: &RpcServiceEnvironment,
    ) -> Result<Self, anyhow::Error> {
        let SnapshotCycleInfo {
            snapshot_block_level,
            snapshot_block_hash,
            ..
        } = snapshot_info;

        let delegate_info_string = get_routed_request(
            &format!("chains/main/blocks/{snapshot_block_level}/context/delegates/{delegate}"),
            snapshot_block_hash.clone(),
            env,
        )
        .await?;

        Ok(serde_json::from_str::<DelegateInfo>(&delegate_info_string)?)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DelegateInfoInt {
    staking_balance: BigInt,
    delegated_contracts: Vec<String>,

    // create delegated balances map
    delegator_balances: BTreeMap<Delegate, BigInt>,
}

impl TryFrom<DelegateInfo> for DelegateInfoInt {
    type Error = anyhow::Error;

    fn try_from(value: DelegateInfo) -> Result<Self, Self::Error> {
        let delegate_info = DelegateInfoInt {
            staking_balance: BigInt::from_str(&value.staking_balance)?,
            delegated_contracts: value.delegated_contracts,
            delegator_balances: BTreeMap::new(),
        };
        Ok(delegate_info)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum OperationKind {
    SeedNonceRevelation,
    DoubleBakingEvidence,
    DoublePreendorsementEvidence,
    DoubleEndorsementEvidence,
    ActivateAccount,
    Ballot,
    Endorsement,
    Preendorsement,
}

#[derive(Clone, Debug, Deserialize)]
struct OperationMetadata {
    balance_updates: Vec<BalanceUpdateKind>,
    delegate: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct BareOperationKind {
    kind: OperationKind,
    metadata: OperationMetadata,
}

#[derive(Clone, Debug, Deserialize)]
struct OperationRepresentation {
    contents: Vec<BareOperationKind>,
}

// TODO: should we use the concrete PublicKeyHash type?
pub type Delegate = String;

#[derive(Clone, Debug, Serialize)]
pub struct DelegateRewardDistribution {
    address: String,
    total_rewards: String,
    staking_balance: String,
    delegator_rewards: Vec<DelegatorInfo>,
}

impl DelegateRewardDistribution {
    fn new(address: String, total_rewards: String, staking_balance: String) -> Self {
        Self {
            address,
            total_rewards,
            staking_balance,
            delegator_rewards: Vec::new(),
        }
    }

    fn insert_delegator_reward(&mut self, delegator_info: DelegatorInfo) {
        self.delegator_rewards.push(delegator_info);
    }
}

#[derive(Clone, Debug, Serialize)]
struct DelegatorInfo {
    address: String,
    balance: String,
    reward: String,
}

impl DelegatorInfo {
    fn new(address: String, balance: String, reward: String) -> Self {
        Self {
            address,
            balance,
            reward,
        }
    }
}

// To serialize bigint we use its string form
// pub type DelegateRewardDistribution = BTreeMap<Delegator, String>;
// pub type CycleRewardDistribution = Vec<DelegateRewardDistribution>;

#[derive(Clone, Debug, Serialize)]
pub struct DelegateRewards {
    address: String,
    total_rewards: String,
    staking_balance: String,
    delegator_count: usize,
}

impl DelegateRewards {
    fn new(delegate: String, cycle_rewards: CycleRewardsInt, delegate_info: DelegateInfo) -> Self {
        // sometimes the delegate itself is present in the delegated contracts
        let delegator_count = if delegate_info.delegated_contracts.contains(&delegate) {
            delegate_info.delegated_contracts.len().saturating_sub(1)
        } else {
            delegate_info.delegated_contracts.len()
        };

        Self {
            address: delegate,
            total_rewards: cycle_rewards.totoal_rewards.to_string(),
            staking_balance: delegate_info.staking_balance,
            delegator_count,
        }
    }
}

pub(crate) async fn get_cycle_delegate_rewards(
    chain_id: &ChainId,
    env: &RpcServiceEnvironment,
    cycle_num: i32,
) -> Result<Vec<DelegateRewards>, anyhow::Error> {
    let current_head_hash = if let Ok(shared_state) = env.state().read() {
        shared_state.current_head().hash.clone()
    } else {
        anyhow::bail!("Cannot access current head")
    };

    let protocol_hash =
        &get_additional_data_or_fail(chain_id, &current_head_hash, env.persistent_storage())?
            .protocol_hash;

    // Note: (Assumption, needs to be verifed) There is a bug in cycle era storage that won't save the era data on a new protocol change if there was no change
    let saved_cycle_era_in_proto_hash =
        ProtocolHash::from_base58_check(&SupportedProtocol::Proto011.protocol_hash())?;
    let cycle_era = get_cycle_era(&saved_cycle_era_in_proto_hash, cycle_num, env)?;
    let (_, end) = cycle_range(&cycle_era, cycle_num);

    let constants = get_constants(protocol_hash, env)?;

    match SupportedProtocol::try_from(protocol_hash)? {
        SupportedProtocol::Proto012 | SupportedProtocol::Proto013 => {
            let end_hash = parse_block_hash(chain_id, &end.to_string(), env)?;

            let cycle_rewards = get_cycle_rewards(cycle_num, &cycle_era, env, chain_id).await?;

            let snapshot_info = SnapshotCycleInfo::fetch_snapshot_cycle(
                cycle_num,
                end_hash,
                end,
                &saved_cycle_era_in_proto_hash,
                chain_id,
                &constants,
                env,
            )
            .await?;

            let mut res: Vec<DelegateRewards> = Vec::with_capacity(cycle_rewards.len());
            for (delegate, delegate_rewards) in cycle_rewards {
                let delegate_info =
                    DelegateInfo::try_get_from_rpc(delegate.clone(), &snapshot_info, env).await?;
                let delegate_rewards_with_delegator_count =
                    DelegateRewards::new(delegate, delegate_rewards, delegate_info);
                res.push(delegate_rewards_with_delegator_count);
            }

            Ok(res)
        }
        _ => Err(UnsupportedProtocolError {
            protocol: protocol_hash.to_string(),
        }
        .into()),
    }
}

// TODO: create proper errors
pub(crate) async fn get_cycle_rewards_distribution(
    chain_id: &ChainId,
    env: &RpcServiceEnvironment,
    cycle_num: i32,
    delegate: &str,
) -> Result<DelegateRewardDistribution, anyhow::Error> {
    let current_head_hash = if let Ok(shared_state) = env.state().read() {
        shared_state.current_head().hash.clone()
    } else {
        anyhow::bail!("Cannot access current head")
    };

    let protocol_hash =
        &get_additional_data_or_fail(chain_id, &current_head_hash, env.persistent_storage())?
            .protocol_hash;

    // Note: (Assumption, needs to be verifed) There is a bug in cycle era storage that won't save the era data on a new protocol change if there was no change
    let saved_cycle_era_in_proto_hash =
        ProtocolHash::from_base58_check(&SupportedProtocol::Proto011.protocol_hash())?;
    let cycle_era = get_cycle_era(&saved_cycle_era_in_proto_hash, cycle_num, env)?;
    let (_, end) = cycle_range(&cycle_era, cycle_num);

    let constants = get_constants(protocol_hash, env)?;

    match SupportedProtocol::try_from(protocol_hash)? {
        SupportedProtocol::Proto012 | SupportedProtocol::Proto013 => {
            let end_hash = parse_block_hash(chain_id, &end.to_string(), env)?;

            let cycle_rewards = get_cycle_rewards(cycle_num, &cycle_era, env, chain_id).await?;

            let snapshot_info = SnapshotCycleInfo::fetch_snapshot_cycle(
                cycle_num,
                end_hash,
                end,
                &saved_cycle_era_in_proto_hash,
                chain_id,
                &constants,
                env,
            )
            .await?;

            if let Some(cycle_rewards) = cycle_rewards.get(delegate) {
                let reward_distributon = get_delegate_reward_distribution(
                    delegate,
                    &cycle_rewards.clone(),
                    &snapshot_info,
                    env,
                )
                .await?;
                Ok(reward_distributon)
            } else {
                bail!(
                    "Baker has not earned any rewards during cycle {}",
                    cycle_num
                );
            }
        }
        _ => Err(UnsupportedProtocolError {
            protocol: protocol_hash.to_string(),
        }
        .into()),
    }
}

#[derive(Debug)]
struct SnapshotCycleInfo {
    snapshot_block_level: i32,
    snapshot_block_hash: BlockHash,
    snapshot_block_hash_predecessor: BlockHash,
    edge_case_data: Option<EdgeCaseData>,
}

/// Structure containing data for further computation of the case when a snapshot index is 15
#[derive(Debug)]
struct EdgeCaseData {
    delegates_with_unrevealed_nonces: Vec<String>,
    missing_endorsers: Vec<DelegateRights>,
}

// TODO: duplicate?
#[derive(Clone, Debug, Default, Deserialize)]
struct EndorsingRights {
    delegates: Vec<DelegateRights>,
}

// TODO: duplicate?
#[derive(Clone, Debug, Default, Deserialize)]
struct DelegateRights {
    delegate: String,
    endorsing_power: i32,
}

impl SnapshotCycleInfo {
    async fn fetch_snapshot_cycle(
        interrogated_cycle: i32,
        block_hash: BlockHash,
        level: i32,
        protocol_hash: &ProtocolHash,
        chain_id: &ChainId,
        constants: &Constants,
        env: &RpcServiceEnvironment,
    ) -> Result<Self, anyhow::Error> {
        // was there a protocol switch in the last PERSERVED CYCLES

        // for the interogated cycle the delegate stuff was set at the end of current_cycle - PRESERVED_CYCLES - 1
        let cycle = interrogated_cycle - constants.preserved_cycles - 1;
        let frozen_cycle_era = get_cycle_era(protocol_hash, cycle, env)?;
        let (first_block_level, _) = cycle_range(&frozen_cycle_era, cycle);

        let snapshot_index = get_routed_request(
            &format!(
                "chains/main/blocks/{level}/context/selected_snapshot?cycle={interrogated_cycle}"
            ),
            block_hash,
            env,
        )
        .await?
        .trim_end_matches('\n')
        .parse::<i32>()?;

        let snapshot_block_level = if let Some(offset_cycle) =
            SnapshotCycleInfo::switched_to_ithaca(
                constants.preserved_cycles,
                chain_id,
                interrogated_cycle,
                &frozen_cycle_era, // TODO: is this correct?
                env,
            )? {
            // Note: we subtract 1 because we want the last block of the previous cycle (last non ithaca block)
            cycle_range(&frozen_cycle_era, offset_cycle).0 - 1
        } else {
            get_snapshot_block(first_block_level, snapshot_index, constants)
        };

        // let snapshot_block_level = get_snapshot_block(first_block_level, snapshot_index, constants);
        let snapshot_block_hash =
            parse_block_hash(chain_id, &snapshot_block_level.to_string(), env)?;
        let snapshot_block_hash_predecessor =
            parse_block_hash(chain_id, &(snapshot_block_level - 1).to_string(), env)?;

        // Handle edge case of snapshot index 15
        let edge_case_data = if snapshot_index == 15 {
            let block_storage = BlockStorage::new(env.persistent_storage());
            let operations_storage = OperationsStorage::new(env.persistent_storage());
            let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());

            // Collect all unrevealed nonces up to the second to last block in the cycle
            let mut unrevealed = get_unrevealed_nonce_delegates(
                cycle,
                &snapshot_block_hash_predecessor,
                snapshot_block_level - 1,
                env,
            )
            .await?;

            // Collect block data for the last block in the snapshot cycle
            // Note: this is needed becouse the context is modified in the last block and the relevant endorsement and nonce data is cleaned up
            // so we have to check them manually
            let (_, json_data, additional_data, ops) = collect_block_data(
                &snapshot_block_hash,
                &block_storage,
                &block_meta_storage,
                &operations_storage,
            )?;
            let converted_ops = ApplyBlockRequest::convert_operations(ops);

            let mut connection = env.tezos_protocol_api().readable_connection().await?;
            let deserialized_operations = deserialize_operations(
                &json_data,
                &additional_data,
                converted_ops,
                &mut connection,
                chain_id,
            )
            .await;

            // Collect endorsing rights for the second to last block, we need this info later to decide wether delegates missed endorsements in the last block
            let endorsing_rights = get_routed_request(
                &format!(
                    "chains/main/blocks/{snapshot_block_level}/helpers/endorsing_rights?level={}",
                    snapshot_block_level - 1
                ),
                snapshot_block_hash.clone(),
                env,
            )
            .await?;

            // Will be later reduced to only the delegates, that missed the endorsement
            let mut missing_endorsers =
                serde_json::from_str::<Vec<EndorsingRights>>(&endorsing_rights)?
                    .get(0)
                    .map(|rights| rights.delegates.clone())
                    .unwrap_or_default();

            // Collect all the delegates that were included as endorsers in the last block of the snapshot cycle
            let endorsers: HashSet<String> = deserialized_operations
                .clone()
                .map(|ops| {
                    ops[0]
                        .iter()
                        .filter_map(|op| {
                            serde_json::from_str::<OperationRepresentation>(op.get())
                                .ok()
                                .map(|deserialized_op| {
                                    deserialized_op
                                        .contents
                                        .iter()
                                        .filter_map(|content| {
                                            if let OperationKind::Endorsement = content.kind {
                                                content.metadata.delegate.clone()
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<HashSet<String>>()
                                })
                        })
                        .flatten()
                        .collect()
                })
                .unwrap_or_default();

            // Retain only the delegates that are not among the actual endorsers
            missing_endorsers.retain(|e| !endorsers.contains(&e.delegate));

            // Collect all the nonce revelation operations included in the last block of the snapshot cycle
            let delegate_nonce_revealed: Vec<String> = deserialized_operations
                .map(|ops| {
                    ops[2]
                        .iter()
                        .filter_map(|op| {
                            serde_json::from_str::<OperationRepresentation>(op.get())
                                .ok()
                                .map(|deserialized_op| {
                                    deserialized_op
                                        .contents
                                        .iter()
                                        .filter_map(|content| {
                                            if let OperationKind::SeedNonceRevelation = content.kind
                                            {
                                                Some(
                                                    content
                                                        .metadata
                                                        .balance_updates
                                                        .iter()
                                                        .filter_map(|balance_update| {
                                                            if let BalanceUpdateKind::Contract(
                                                                contract_update,
                                                            ) = balance_update
                                                            {
                                                                Some(
                                                                    contract_update
                                                                        .contract
                                                                        .clone(),
                                                                )
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<Vec<String>>(),
                                                )
                                            } else {
                                                None
                                            }
                                        })
                                        .flatten()
                                        .collect::<Vec<String>>()
                                })
                        })
                        .flatten()
                        .collect()
                })
                .unwrap_or_default();

            // If a delegate revealed its nonce in the last block of the snapshot cycle, remove it from the delegates with unrevealed nonces
            unrevealed.retain(|val| !delegate_nonce_revealed.contains(val));

            Some(EdgeCaseData {
                delegates_with_unrevealed_nonces: unrevealed,
                missing_endorsers,
            })
        } else {
            None
        };

        Ok(Self {
            snapshot_block_level,
            snapshot_block_hash,
            snapshot_block_hash_predecessor,
            edge_case_data,
        })
    }
    fn switched_to_ithaca(
        preserved_cycles: i32,
        chain_id: &ChainId,
        interrogated_cycle: i32,
        cycle_era: &CycleEra,
        env: &RpcServiceEnvironment,
    ) -> Result<Option<i32>, anyhow::Error> {
        let ithaca_protocol_hash =
            ProtocolHash::from_base58_check(&SupportedProtocol::Proto012.protocol_hash())?;
        let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());
        for cycle_offset in (0..=preserved_cycles).rev() {
            let offset_cycle = interrogated_cycle - cycle_offset;
            let (offset_cycle_start, _) = cycle_range(cycle_era, offset_cycle);

            let offset_cycle_start_predecessor_hash =
                parse_block_hash(chain_id, &(offset_cycle_start - 1).to_string(), env)?;

            if let Some(additional_data) =
                block_meta_storage.get_additional_data(&offset_cycle_start_predecessor_hash)?
            {
                // immediatly short circuit if the protocol hash is already on ithaca
                if additional_data.protocol_hash == ithaca_protocol_hash {
                    return Ok(None);
                }
                if let Some(new_porotocol_hash) = additional_data.is_protocol_switch() {
                    if new_porotocol_hash == ithaca_protocol_hash {
                        return Ok(Some(offset_cycle));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EndorsementParticipation {
    remaining_allowed_missed_slots: i32,
    expected_endorsing_rewards: String,
}

async fn get_delegate_reward_distribution(
    delegate: &str,
    rewards: &CycleRewardsInt,
    snapshot_info: &SnapshotCycleInfo,
    env: &RpcServiceEnvironment,
) -> Result<DelegateRewardDistribution, anyhow::Error> {
    let SnapshotCycleInfo {
        snapshot_block_level,
        snapshot_block_hash,
        snapshot_block_hash_predecessor,
        edge_case_data,
        ..
    } = snapshot_info;

    let delegate_info_string = get_routed_request(
        &format!("chains/main/blocks/{snapshot_block_level}/context/delegates/{delegate}"),
        snapshot_block_hash.clone(),
        env,
    )
    .await?;
    let mut delegate_info: DelegateInfoInt =
        serde_json::from_str::<DelegateInfo>(&delegate_info_string)?.try_into()?;

    // Edge case when the snapshot index is 15. The snapshot is taken before the endorsing rewards are distributed!
    let staking_balance = if let Some(edge_case_data) = edge_case_data {
        let endorsing_data_at_snapshot = get_routed_request(
            &format!(
                "chains/main/blocks/{}/context/delegates/{delegate}/participation",
                snapshot_block_level - 1
            ),
            snapshot_block_hash_predecessor.clone(),
            env,
        )
        .await?;
        let endorsing_data_at_snapshot: EndorsementParticipation =
            serde_json::from_str::<EndorsementParticipation>(&endorsing_data_at_snapshot)?;

        // if the delegate missed the endorsement in the last block adjust the endorsing reward accordingly
        let endorsement_reward = if let Some(delegate_endorsing_rights) = edge_case_data
            .missing_endorsers
            .iter()
            .find(|d| d.delegate == delegate)
        {
            // this is the last endorsement of the cycle, so checking wether this last endorsement power surpassed the
            // remaining_allowed_missed_slots is sufficient
            if endorsing_data_at_snapshot.remaining_allowed_missed_slots
                > delegate_endorsing_rights.endorsing_power
            {
                BigInt::from_str("0")?
            } else {
                BigInt::from_str(&endorsing_data_at_snapshot.expected_endorsing_rewards)?
            }
        } else {
            BigInt::from_str(&endorsing_data_at_snapshot.expected_endorsing_rewards)?
        };

        // was delegates nonce revealed?
        if edge_case_data
            .delegates_with_unrevealed_nonces
            .contains(&delegate.to_string())
        {
            // if not the endorsing reward was not added, we are good
            delegate_info.staking_balance
        } else {
            // the endorsing reward was added, subtract
            delegate_info.staking_balance - endorsement_reward
        }
    } else {
        delegate_info.staking_balance
    };

    let mut reward_distributon = DelegateRewardDistribution::new(
        delegate.to_string(),
        rewards.totoal_rewards.to_string(),
        staking_balance.to_string(),
    );

    let delegators = delegate_info.delegated_contracts.clone();
    let mut delegator_balance_sum: BigInt = BigInt::new(Sign::Plus, vec![0]);
    for delegator in delegators {
        // ignore the delegate itself as it is part of the list
        if delegator == delegate {
            continue;
        }
        let delegator_balance = get_routed_request(
            &format!("chains/main/blocks/{snapshot_block_level}/context/raw/json/contracts/index/{delegator}/balance"),
            snapshot_block_hash.clone(),
            env
        ).await?;
        let delegator_balance = delegator_balance
            .trim_end_matches('\n')
            .trim_matches('\"')
            .parse::<BigInt>()
            .ok()
            .unwrap_or_else(|| BigInt::new(Sign::Plus, vec![0]));

        let delegator_reward_share = get_delegator_reward_share(
            staking_balance.clone(),
            rewards.totoal_rewards.clone(),
            delegator_balance.clone(),
        );
        let delegator_info = DelegatorInfo::new(
            delegator.clone(),
            delegator_balance.to_string(),
            delegator_reward_share.to_string(),
        );
        reward_distributon.insert_delegator_reward(delegator_info);

        delegator_balance_sum += delegator_balance.clone();
        delegate_info
            .delegator_balances
            .insert(delegator, delegator_balance);
    }

    Ok(reward_distributon)
}

fn get_cycle_era(
    protocol_hash: &ProtocolHash,
    cycle_num: i32,
    env: &RpcServiceEnvironment,
) -> Result<CycleEra, anyhow::Error> {
    if let Some(eras) = CycleErasStorage::new(env.persistent_storage()).get(protocol_hash)? {
        if let Some(era) = eras.into_iter().find(|era| era.first_cycle() < &cycle_num) {
            Ok(era)
        } else {
            anyhow::bail!("No matching cycle era found")
        }
    } else {
        anyhow::bail!("No saved cycle eras found for protocol")
    }
}

fn get_snapshot_block(start_block_level: i32, snapshot_index: i32, constants: &Constants) -> i32 {
    let blocks_per_stake_snapshot = constants.blocks_per_stake_snapshot;

    start_block_level + (snapshot_index + 1) * blocks_per_stake_snapshot - 1
}

fn cycle_range(era: &CycleEra, cycle_num: i32) -> (i32, i32) {
    let cycle_offset = cycle_num - *era.first_cycle();

    let start = *era.first_level() + cycle_offset * *era.blocks_per_cycle();
    let end = start + *era.blocks_per_cycle() - 1;

    (start, end)
}

async fn get_routed_request(
    path: &str,
    block_hash: BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<String, anyhow::Error> {
    let meth = RpcMethod::GET;

    let body = String::from("");

    let req = RpcRequest {
        body,
        context_path: String::from(path.trim_end_matches('/')),
        meth,
        content_type: None,
        accept: None,
    };
    let chain_id = parse_chain_id(MAIN_CHAIN_ID, env)?;

    let res = protocol::call_protocol_rpc(MAIN_CHAIN_ID, chain_id, block_hash, req, env).await?;

    Ok(res.1.clone())
}

// TODO: comisson
fn get_delegator_reward_share(
    staking_balance: BigInt,
    delegate_total_reward: BigInt,
    delegator_balance: BigInt,
) -> BigInt {
    let share = BigRational::new(delegator_balance, staking_balance);

    let reward = BigRational::from(delegate_total_reward) * share;

    reward.round().to_integer()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum NonceArrayElement {
    Level(i32),
    RevealedNonce(String),
    UnrevealedNonce(Vec<String>),
}

async fn get_unrevealed_nonce_delegates(
    cycle: i32,
    block_hash: &BlockHash,
    block_level: i32,
    env: &RpcServiceEnvironment,
) -> Result<Vec<String>, anyhow::Error> {
    // Nonce revelations in a cycle means that the baker should reveal his nonces created in the previous cycle
    let nonces = get_routed_request(
        &format!(
            "chains/main/blocks/{block_level}/context/raw/json/cycle/{}/nonces?depth=2",
            cycle - 1
        ),
        block_hash.clone(),
        env,
    )
    .await?;

    let nonces_raw: Vec<Vec<NonceArrayElement>> = serde_json::from_str(&nonces)?;

    let unrevealed: Vec<String> = nonces_raw
        .iter()
        .map(|nonce| {
            Some(
                nonce
                    .iter()
                    .filter_map(|element| {
                        if let NonceArrayElement::UnrevealedNonce(unrevealed) = element {
                            unrevealed.get(1).cloned()
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<String>>(),
            )
        })
        .flatten()
        .flatten()
        .collect();

    Ok(unrevealed)
}

async fn deserialize_operations(
    json_data: &BlockJsonData,
    additional_data: &BlockAdditionalData,
    converted_ops: Vec<Vec<Operation>>,
    connection: &mut ProtocolRunnerConnection,
    chain_id: &ChainId,
) -> Option<BlockOperations> {
    let response = connection
        .apply_block_operations_metadata(
            chain_id.clone(),
            converted_ops,
            json_data.operations_proto_metadata_bytes.clone(),
            additional_data.protocol_hash.clone(),
            additional_data.next_protocol_hash.clone(),
        )
        .await;

    let response = if let Ok(response) = response {
        response
    } else {
        return None;
    };

    Some(serde_json::from_str(&response).unwrap_or_default())
}

type StorageData = (
    BlockHeaderWithHash,
    BlockJsonData,
    BlockAdditionalData,
    Vec<OperationsForBlocksMessage>,
);

fn collect_block_data(
    block_hash: &BlockHash,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    operations_storage: &OperationsStorage,
) -> Result<StorageData, anyhow::Error> {
    if let Some((block_header_with_hash, block_json_data)) =
        block_storage.get_with_json_data(block_hash)?
    {
        if let Some(block_additional_data) =
            block_meta_storage.get_additional_data(&block_header_with_hash.hash)?
        {
            let operations_data =
                operations_storage.get_operations(&block_header_with_hash.hash)?;
            Ok((
                block_header_with_hash,
                block_json_data,
                block_additional_data,
                operations_data,
            ))
        } else {
            anyhow::bail!("No addtional data found")
        }
    } else {
        anyhow::bail!("No block data found")
    }
}

async fn get_cycle_rewards(
    cycle_num: i32,
    cycle_era: &CycleEra,
    env: &RpcServiceEnvironment,
    chain_id: &ChainId,
) -> Result<BTreeMap<Delegate, CycleRewardsInt>, anyhow::Error> {
    let reward_storage = RewardStorage::new(env.persistent_storage());

    if let Some(rewards) = reward_storage.get(&cycle_num)? {
        Ok(rewards)
    } else {
        let rewards = collect_cycle_rewards(cycle_num, cycle_era, env, chain_id).await?;
        reward_storage.put(&cycle_num, rewards.clone())?;
        Ok(rewards)
    }
}

async fn collect_cycle_rewards(
    cycle_num: i32,
    cycle_era: &CycleEra,
    env: &RpcServiceEnvironment,
    chain_id: &ChainId,
) -> Result<BTreeMap<Delegate, CycleRewardsInt>, anyhow::Error> {
    let mut result: BTreeMap<Delegate, CycleRewardsInt> = BTreeMap::new();

    let block_meta_storage = BlockMetaStorage::new(env.persistent_storage());
    let block_storage = BlockStorage::new(env.persistent_storage());
    let operations_storage = OperationsStorage::new(env.persistent_storage());

    let (start, end) = cycle_range(cycle_era, cycle_num);

    let mut blocks: Vec<(
        BlockHeaderWithHash,
        BlockJsonData,
        BlockAdditionalData,
        Vec<OperationsForBlocksMessage>,
    )> = Vec::with_capacity(*cycle_era.blocks_per_cycle() as usize);

    // get all the data needed from storage
    for level in start..=end {
        let hash = parse_block_hash(chain_id, &level.to_string(), env)?;
        if let Some((block_header_with_hash, block_json_data)) =
            block_storage.get_with_json_data(&hash)?
        {
            if let Some(block_additional_data) =
                block_meta_storage.get_additional_data(&block_header_with_hash.hash)?
            {
                let operations_data =
                    operations_storage.get_operations(&block_header_with_hash.hash)?;
                blocks.push((
                    block_header_with_hash,
                    block_json_data,
                    block_additional_data,
                    operations_data,
                ));
            }
        }
    }

    let mut connection = env.tezos_protocol_api().readable_connection().await?;
    for (block_header, block_json_data, block_additional_data, operations_data) in blocks {
        let response = connection
            .apply_block_result_metadata(
                block_header.header.context().clone(),
                block_json_data.block_header_proto_metadata_bytes,
                block_additional_data.max_operations_ttl().into(),
                block_additional_data.protocol_hash.clone(),
                block_additional_data.next_protocol_hash.clone(),
            )
            .await;

        let response = if let Ok(response) = response {
            response
        } else {
            continue;
        };

        let metadata: BlockMetadata = serde_json::from_str(&response).unwrap_or_default();

        let converted_ops = ApplyBlockRequest::convert_operations(operations_data);

        // Optimalization: Deserialize the operations only when the anonymous validation pass is not empty
        // Further optimalization would be the ability to deserialize only one validation pass
        let block_operations: Option<BlockOperations> = if !converted_ops[2].is_empty() {
            let response = connection
                .apply_block_operations_metadata(
                    chain_id.clone(),
                    converted_ops,
                    block_json_data.operations_proto_metadata_bytes,
                    block_additional_data.protocol_hash.clone(),
                    block_additional_data.next_protocol_hash.clone(),
                )
                .await;

            let response = if let Ok(response) = response {
                response
            } else {
                continue;
            };

            Some(serde_json::from_str(&response).unwrap_or_default())
        } else {
            None
        };

        if let Some(balance_updates) = metadata.get("balance_updates") {
            if let Some(balance_updates_array) = balance_updates.as_array() {
                for balance_update in balance_updates_array {
                    // deserialize
                    let balance_update: BalanceUpdateKind =
                        serde_json::from_value(balance_update.clone()).unwrap_or_default();

                    match balance_update.clone() {
                        BalanceUpdateKind::Contract(contract_updates) => {
                            let entry = result
                                .entry(contract_updates.contract.clone())
                                .or_insert_with(CycleRewardsInt::default);
                            let change = BigInt::from_str(&contract_updates.change)?;
                            entry.totoal_rewards += change.clone();
                        }
                        // The contract balance_update subtracts the deposit, we just add back the deposited amount
                        BalanceUpdateKind::Freezer(freezer_update) => {
                            if let BalanceUpdateCategory::Deposits = freezer_update.category {
                                result
                                    .entry(freezer_update.delegate.clone())
                                    .or_insert_with(CycleRewardsInt::default)
                                    .totoal_rewards += BigInt::from_str(&freezer_update.change)?;
                            }
                        }
                        _ => { /* Ignore other receipts */ }
                    }
                }
            } else {
                anyhow::bail!("Balance updates not an array");
            }
        } else {
            anyhow::bail!("Balance updates not found");
        };

        if let Some(block_operations) = block_operations {
            for operations in &block_operations[2] {
                let operation: OperationRepresentation = serde_json::from_str(operations.get())?;

                for content in operation.contents {
                    match content.kind {
                        OperationKind::DoubleBakingEvidence
                        | OperationKind::DoubleEndorsementEvidence
                        | OperationKind::DoublePreendorsementEvidence
                        | OperationKind::SeedNonceRevelation => {
                            for balance_update in content.metadata.balance_updates {
                                if let BalanceUpdateKind::Contract(contract_updates) =
                                    balance_update
                                {
                                    result
                                        .entry(contract_updates.contract.clone())
                                        .or_insert_with(CycleRewardsInt::default)
                                        .totoal_rewards +=
                                        BigInt::from_str(&contract_updates.change)?;
                                }
                            }
                        }
                        _ => { /* Ignore */ }
                    }
                }
            }
        }
    }

    // trim all delegates that has 0 rewards (deposit changes could still occur after being inactive)
    // so only active delegates will be checked
    result.retain(|_, reward| reward.totoal_rewards != BigInt::from(0));
    Ok(result)
}

/// The requred constants for rewards calculation
#[derive(Clone, Debug, Deserialize)]
struct Constants {
    preserved_cycles: i32,
    blocks_per_stake_snapshot: i32,
}

fn get_constants(
    protocol_hash: &ProtocolHash,
    env: &RpcServiceEnvironment,
) -> Result<Constants, anyhow::Error> {
    let constants_storage = ConstantsStorage::new(env.persistent_storage());

    let constants = constants_storage.get(protocol_hash)?;

    if let Some(constants_string) = constants {
        Ok(serde_json::from_str(&constants_string)?)
    } else {
        anyhow::bail!("No constants found for protocol {protocol_hash}")
    }
}
