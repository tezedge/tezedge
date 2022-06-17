// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::BTreeMap,
    convert::TryInto,
    io, mem,
    num::ParseIntError,
    str::{self, FromStr},
    sync::mpsc,
    thread,
    time::Duration,
};

use derive_more::From;
use reqwest::{
    blocking::{Client, ClientBuilder},
    StatusCode, Url,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crypto::hash::{
    BlockHash, BlockPayloadHash, ChainId, ContextHash, ContractTz1Hash, NonceHash, OperationHash,
    OperationListListHash, ProtocolHash, Signature,
};
use tezos_encoding::{binary_reader::BinaryReaderError, types::SizedBytes};
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};
use tezos_messages::{
    p2p::{
        binary_message::BinaryRead,
        encoding::{fitness::Fitness, operation::DecodedOperation},
    },
    Timestamp,
};

#[cfg(feature = "fuzzing")]
use tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator;

use super::event::{Block, OperationSimple, Slots};
use crate::machine::{BakerAction, OperationsEventAction, ProposalEventAction, RpcErrorAction};

#[derive(Clone)]
pub struct RpcClient {
    tx: mpsc::Sender<BakerAction>,
    endpoint: Url,
    inner: Client,
}

#[derive(Debug, Error)]
pub enum RpcError {
    #[error("{inner}, url: {url}, body: {body}")]
    WithBody {
        url: Url,
        body: String,
        inner: RpcErrorInner,
    },
    #[error("{inner}, url: {url}")]
    WithContext { url: Url, inner: RpcErrorInner },
    #[error("{_0}")]
    Less(RpcErrorInner),
}

impl AsRef<RpcErrorInner> for RpcError {
    fn as_ref(&self) -> &RpcErrorInner {
        match self {
            RpcError::WithBody { ref inner, .. } => inner,
            RpcError::WithContext { ref inner, .. } => inner,
            RpcError::Less(ref inner) => inner,
        }
    }
}

#[derive(Debug, Error, From)]
pub enum RpcErrorInner {
    #[error("reqwest: {_0}")]
    Reqwest(reqwest::Error),
    #[error("serde_json: {_0}")]
    SerdeJson(serde_json::Error),
    #[error("io: {_0}")]
    Io(io::Error),
    #[error("hex: {_0}")]
    Hex(hex::FromHexError),
    #[error("nom: {_0}")]
    Nom(BinaryReaderError),
    #[error("utf8: {_0}")]
    Utf8(str::Utf8Error),
    #[error("parse: {_0}, {_1}")]
    IntParse(ParseIntError, String),
    #[error("time parse error: {_0}")]
    Chrono(time::error::Parse),
    #[error("node: {_0}")]
    NodeError(String, StatusCode),
    #[error("invalid fitness")]
    InvalidFitness,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Protocol {
    Ithaca,
    Jakarta,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::Ithaca
    }
}

impl FromStr for Protocol {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "i" | "ithaca" | "ithacanet" => Ok(Protocol::Ithaca),
            "j" | "jakarta" | "jakartanet" => Ok(Protocol::Jakarta),
            _ => Err("unknown protocol".to_string()),
        }
    }
}

#[derive(Deserialize)]
pub struct ShellBlockShortHeader {
    level: i32,
    proto: u8,
    predecessor: BlockHash,
    timestamp: String,
    validation_pass: u8,
    operations_hash: OperationListListHash,
    fitness: Vec<String>,
    context: ContextHash,
}

pub trait ProtocolHeaderFull
where
    Self: BinWriter + std::fmt::Debug,
{
    fn payload_round(&self) -> i32;

    fn level(&self) -> i32;

    fn set_payload_hash(&mut self, payload_hash: BlockPayloadHash);

    fn set_proof_of_work_nonce(&mut self, pow: SizedBytes<8>);
}

pub trait ProtocolHeader {
    type FullHeader: ProtocolHeaderFull + BinWriter;

    const NAME: &'static str;

    const POW_OFFSET: usize;

    fn new(
        payload_hash: BlockPayloadHash,
        payload_round: i32,
        seed_nonce_hash: Option<NonceHash>,
        liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    ) -> Self;

    fn payload_hash(&self) -> &BlockPayloadHash;

    fn payload_round(&self) -> i32;

    fn proof_of_work_nonce(&self) -> SizedBytes<8>;

    fn full_header(
        self,
        shell_header: ShellBlockShortHeader,
    ) -> Result<Self::FullHeader, RpcErrorInner>;
}

// signature watermark: 0x11 | chain_id
#[derive(BinWriter, HasEncoding, NomReader, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct ProtocolBlockHeaderI {
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct FullHeaderI {
    #[encoding(builtin = "Int32")]
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: Timestamp,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Fitness,
    pub context: ContextHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

impl ProtocolHeaderFull for FullHeaderI {
    fn payload_round(&self) -> i32 {
        self.payload_round
    }

    fn level(&self) -> i32 {
        self.level
    }

    fn set_payload_hash(&mut self, payload_hash: BlockPayloadHash) {
        self.payload_hash = payload_hash;
    }

    fn set_proof_of_work_nonce(&mut self, pow: SizedBytes<8>) {
        self.proof_of_work_nonce = pow;
    }
}

impl ProtocolHeader for ProtocolBlockHeaderI {
    type FullHeader = FullHeaderI;

    const NAME: &'static str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

    const POW_OFFSET: usize = mem::size_of::<BlockPayloadHash>() + mem::size_of::<i32>();

    fn new(
        payload_hash: BlockPayloadHash,
        payload_round: i32,
        seed_nonce_hash: Option<NonceHash>,
        liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    ) -> Self {
        let l = matches!(liquidity_baking_toggle_vote, LiquidityBakingToggleVote::On);
        ProtocolBlockHeaderI {
            payload_hash,
            payload_round,
            proof_of_work_nonce: SizedBytes(0x7985fafe1fb70300u64.to_be_bytes()),
            seed_nonce_hash,
            liquidity_baking_escape_vote: l,
            // fake signature
            signature: Signature(vec![0; 64]),
        }
    }

    fn payload_hash(&self) -> &BlockPayloadHash {
        &self.payload_hash
    }

    fn payload_round(&self) -> i32 {
        self.payload_round
    }

    fn proof_of_work_nonce(&self) -> SizedBytes<8> {
        self.proof_of_work_nonce.clone()
    }

    fn full_header(
        self,
        shell_header: ShellBlockShortHeader,
    ) -> Result<Self::FullHeader, RpcErrorInner> {
        let ShellBlockShortHeader {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness,
            context,
        } = shell_header;
        let ProtocolBlockHeaderI {
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            ..
        } = self;
        let timestamp = OffsetDateTime::parse(&timestamp, &Rfc3339)?
            .unix_timestamp()
            .into();
        Ok(FullHeaderI {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness: {
                let mut v = vec![];
                for fitness_str in fitness {
                    let item = hex::decode(fitness_str)?;
                    v.push(item);
                }
                v.into()
            },
            context,
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_escape_vote,
            signature: Signature(vec![0; 64]),
        })
    }
}

#[derive(BinWriter, HasEncoding, NomReader, Clone, Copy, Debug, Serialize, Deserialize)]
#[encoding(tags = "u8")]
#[serde(rename_all = "snake_case")]
pub enum LiquidityBakingToggleVote {
    On,
    Off,
    Pass,
}

impl Default for LiquidityBakingToggleVote {
    fn default() -> Self {
        LiquidityBakingToggleVote::Off
    }
}

impl FromStr for LiquidityBakingToggleVote {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "on" => Ok(LiquidityBakingToggleVote::On),
            "off" => Ok(LiquidityBakingToggleVote::Off),
            "pass" => Ok(LiquidityBakingToggleVote::Pass),
            _ => Err("unknown value".to_string()),
        }
    }
}

#[derive(BinWriter, HasEncoding, NomReader, Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct ProtocolBlockHeaderJ {
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    pub signature: Signature,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, HasEncoding, NomReader, BinWriter)]
pub struct FullHeaderJ {
    #[encoding(builtin = "Int32")]
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: Timestamp,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Fitness,
    pub context: ContextHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    pub signature: Signature,
}

impl ProtocolHeaderFull for FullHeaderJ {
    fn payload_round(&self) -> i32 {
        self.payload_round
    }

    fn level(&self) -> i32 {
        self.level
    }

    fn set_payload_hash(&mut self, payload_hash: BlockPayloadHash) {
        self.payload_hash = payload_hash;
    }

    fn set_proof_of_work_nonce(&mut self, pow: SizedBytes<8>) {
        self.proof_of_work_nonce = pow;
    }
}

impl ProtocolHeader for ProtocolBlockHeaderJ {
    type FullHeader = FullHeaderJ;

    const NAME: &'static str = "PtJakart2xVj7pYXJBXrqHgd82rdkLey5ZeeGwDgPp9rhQUbSqY";

    const POW_OFFSET: usize = mem::size_of::<BlockPayloadHash>() + mem::size_of::<i32>();

    fn new(
        payload_hash: BlockPayloadHash,
        payload_round: i32,
        seed_nonce_hash: Option<NonceHash>,
        liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    ) -> Self {
        ProtocolBlockHeaderJ {
            payload_hash,
            payload_round,
            proof_of_work_nonce: SizedBytes(0x7985fafe1fb70300u64.to_be_bytes()),
            seed_nonce_hash,
            liquidity_baking_toggle_vote,
            // fake signature
            signature: Signature(vec![0; 64]),
        }
    }

    fn payload_hash(&self) -> &BlockPayloadHash {
        &self.payload_hash
    }

    fn payload_round(&self) -> i32 {
        self.payload_round
    }

    fn proof_of_work_nonce(&self) -> SizedBytes<8> {
        self.proof_of_work_nonce.clone()
    }

    fn full_header(
        self,
        shell_header: ShellBlockShortHeader,
    ) -> Result<Self::FullHeader, RpcErrorInner> {
        let ShellBlockShortHeader {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness,
            context,
        } = shell_header;
        let ProtocolBlockHeaderJ {
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_toggle_vote,
            ..
        } = self;
        let timestamp = OffsetDateTime::parse(&timestamp, &Rfc3339)?
            .unix_timestamp()
            .into();
        Ok(FullHeaderJ {
            level,
            proto,
            predecessor,
            timestamp,
            validation_pass,
            operations_hash,
            fitness: {
                let mut v = vec![];
                for fitness_str in fitness {
                    let item = hex::decode(fitness_str)?;
                    v.push(item);
                }
                v.into()
            },
            context,
            payload_hash,
            payload_round,
            proof_of_work_nonce,
            seed_nonce_hash,
            liquidity_baking_toggle_vote,
            signature: Signature(vec![0; 64]),
        })
    }
}

pub struct Constants {
    pub nonce_length: usize,
    pub blocks_per_cycle: u32,
    pub blocks_per_commitment: u32,
    pub consensus_committee_size: u32,
    pub proof_of_work_threshold: u64,
    pub minimal_block_delay: Duration,
    pub delay_increment_per_round: Duration,
}

impl RpcClient {
    pub fn new(endpoint: Url, tx: mpsc::Sender<BakerAction>) -> Self {
        RpcClient {
            tx,
            endpoint,
            inner: ClientBuilder::new()
                .timeout(None)
                .build()
                .expect("client should created"),
        }
    }

    pub fn get_chain_id(&self) -> Result<ChainId, RpcError> {
        let url = self
            .endpoint
            .join("chains/main/chain_id")
            .expect("valid constant url");
        self.single_response_blocking(&url, None, None)
    }

    /// nothing to do until bootstrapped, so let's wait synchronously
    pub fn wait_bootstrapped(&self) -> Result<BlockHash, RpcError> {
        let url = self
            .endpoint
            .join("monitor/bootstrapped")
            .expect("valid constant url");

        #[derive(Deserialize)]
        struct Bootstrapped {
            block: BlockHash,
            #[allow(dead_code)]
            timestamp: String,
        }
        let Bootstrapped { block, .. } = self.single_response_blocking(&url, None, None)?;

        Ok(block)
    }

    pub fn get_constants(&self) -> Result<Constants, RpcError> {
        #[derive(Deserialize, Debug)]
        struct ConstantsInner {
            nonce_length: usize,
            blocks_per_cycle: u32,
            blocks_per_commitment: u32,
            consensus_committee_size: u32,
            minimal_block_delay: String,
            delay_increment_per_round: String,
            proof_of_work_threshold: String,
        }

        let url = self
            .endpoint
            .join("chains/main/blocks/head/context/constants")
            .expect("valid constant url");
        let ConstantsInner {
            nonce_length,
            blocks_per_cycle,
            blocks_per_commitment,
            consensus_committee_size,
            minimal_block_delay,
            delay_increment_per_round,
            proof_of_work_threshold,
        } = self.single_response_blocking::<ConstantsInner>(&url, None, None)?;

        Ok(Constants {
            nonce_length,
            blocks_per_cycle,
            blocks_per_commitment,
            consensus_committee_size,
            proof_of_work_threshold: u64::from_be_bytes(
                proof_of_work_threshold
                    .parse::<i64>()
                    .map_err(|err| RpcErrorInner::IntParse(err, "pow threshold".to_string()))
                    .map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?
                    .to_be_bytes(),
            ),
            minimal_block_delay: Duration::from_secs(
                minimal_block_delay
                    .parse()
                    .map_err(|err| RpcErrorInner::IntParse(err, "minimal block delay".to_string()))
                    .map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?,
            ),
            delay_increment_per_round: Duration::from_secs(
                delay_increment_per_round
                    .parse()
                    .map_err(|err| RpcErrorInner::IntParse(err, "delay increment".to_string()))
                    .map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?,
            ),
        })
    }

    pub fn validators(&self, level: i32) -> Result<BTreeMap<ContractTz1Hash, Slots>, RpcError> {
        let mut url = self
            .endpoint
            .join("chains/main/blocks/head/helpers/validators")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("level", &level.to_string());

        #[derive(Deserialize)]
        struct Validator {
            delegate: ContractTz1Hash,
            slots: Vec<u16>,
        }

        let validators = self.single_response_blocking::<Vec<_>>(&url, None, None)?;
        let validators = validators
            .into_iter()
            .map(|Validator { delegate, slots }| (delegate, Slots(slots)))
            .collect();
        Ok(validators)
    }

    pub fn monitor_heads<H>(&self, chain_id: &ChainId) -> Result<(), RpcError>
    where
        H: ProtocolHeader + BinaryRead,
    {
        let s = format!("monitor/heads/{chain_id}");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut().append_pair("next_protocol", H::NAME);

        #[allow(dead_code)]
        #[derive(Deserialize)]
        struct BlockHeaderJsonGeneric {
            hash: BlockHash,
            level: i32,
            proto: u8,
            predecessor: BlockHash,
            timestamp: String,
            validation_pass: u8,
            operations_hash: OperationListListHash,
            fitness: Vec<String>,
            context: ContextHash,
            protocol_data: String,
        }

        let this = self.clone();
        let moved_url = url.clone();
        self.multiple_responses::<BlockHeaderJsonGeneric, _>(&url, None, move |header| {
            let url = moved_url.clone();
            let timestamp =
                convert_timestamp(&header.timestamp).map_err(|inner| RpcError::WithContext {
                    url: url.clone(),
                    inner,
                })?;

            #[derive(Deserialize)]
            struct Protocols {
                protocol: ProtocolHash,
            }

            let s = format!("chains/main/blocks/{}/protocols", header.hash);
            let url = this.endpoint.join(&s).expect("valid url");
            let Protocols { protocol } =
                this.single_response_blocking(&url, None, Some(Duration::from_secs(30)))?;

            let transition = (protocol.to_base58_check() != ProtocolBlockHeaderI::NAME)
                && (protocol.to_base58_check() != ProtocolBlockHeaderJ::NAME);

            let (payload_hash, payload_round, round) = if !transition {
                let protocol_data_bytes = hex::decode(header.protocol_data)
                    .map_err(RpcErrorInner::Hex)
                    .map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?;
                let protocol_header = H::from_bytes(&protocol_data_bytes)
                    .map_err(RpcErrorInner::Nom)
                    .map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?;
                let round =
                    convert_fitness(&header.fitness).map_err(|inner| RpcError::WithContext {
                        url: url.clone(),
                        inner,
                    })?;

                (
                    protocol_header.payload_hash().clone(),
                    protocol_header.payload_round(),
                    round,
                )
            } else {
                (BlockPayloadHash(vec![0x55; 32]), 0, 0)
            };

            Ok(BakerAction::ProposalEvent(ProposalEventAction {
                block: Block {
                    hash: header.hash,
                    level: header.level,
                    predecessor: header.predecessor,
                    timestamp,
                    payload_hash,
                    payload_round,
                    round,
                    transition,
                },
            }))
        })
        .map_err(|inner| RpcError::WithContext { url, inner })
    }

    pub fn get_operations_for_block(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Vec<Vec<OperationSimple>>, RpcError> {
        let s = format!("chains/main/blocks/{block_hash}/operations");
        let url = self.endpoint.join(&s).expect("valid url");
        self.single_response_blocking(&url, None, Some(Duration::from_secs(30)))
    }

    pub fn get_live_blocks(&self, block_hash: &BlockHash) -> Result<Vec<BlockHash>, RpcError> {
        let s = format!("chains/main/blocks/{block_hash}/live_blocks");
        let url = self.endpoint.join(&s).expect("valid url");
        self.single_response_blocking(&url, None, Some(Duration::from_secs(30)))
    }

    pub fn monitor_operations(&self, timeout: Duration) -> Result<(), RpcError> {
        let mut url = self
            .endpoint
            .join("chains/main/mempool/monitor_operations")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("applied", "yes")
            .append_pair("refused", "no")
            .append_pair("outdated", "no")
            .append_pair("branch_refused", "no")
            .append_pair("branch_delayed", "yes");
        self.multiple_responses(&url, Some(timeout), move |operations| {
            Ok(BakerAction::OperationsEvent(OperationsEventAction {
                operations,
            }))
        })
        .map_err(|inner| RpcError::WithContext { url, inner })
    }

    pub fn inject_operation(
        &self,
        chain_id: &ChainId,
        op_hex: String,
        is_async: bool,
    ) -> Result<OperationHash, RpcError> {
        let mut url = self
            .endpoint
            .join("injection/operation")
            .expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("chain", &chain_id.to_base58_check());
        if is_async {
            url.query_pairs_mut().append_key_only("async");
        }
        let body = format!("{op_hex:?}");
        self.single_response_blocking::<OperationHash>(&url, Some(body.clone()), None)
            .map_err(|inner| match inner {
                RpcError::WithContext { url, inner } => RpcError::WithBody { url, body, inner },
                _ => unreachable!(),
            })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn preapply_block<H>(
        &self,
        payload_hash: BlockPayloadHash,
        payload_round: i32,
        seed_nonce_hash: &Option<NonceHash>,
        predecessor_hash: BlockHash,
        timestamp: i64,
        mut operations: [Vec<OperationSimple>; 4],
        liquidity_baking_toggle_vote: LiquidityBakingToggleVote,
    ) -> Result<(Box<dyn ProtocolHeaderFull>, Vec<serde_json::Value>), RpcError>
    where
        H: ProtocolHeader + Serialize,
        H::FullHeader: 'static,
    {
        #[derive(Serialize)]
        struct BlockData {
            protocol_data: serde_json::Value,
            operations: [Vec<OperationSimple>; 4],
        }

        #[derive(Deserialize)]
        struct PreapplyResponse {
            shell_header: ShellBlockShortHeader,
            operations: Vec<serde_json::Value>,
        }

        let protocol_header = H::new(
            payload_hash,
            payload_round,
            seed_nonce_hash.clone(),
            liquidity_baking_toggle_vote,
        );

        let mut protocol_data = serde_json::to_value(&protocol_header)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        let protocol_block_header_obj = protocol_data
            .as_object_mut()
            .expect("`ProtocolBlockHeader` is a structure");
        let proof_of_work_str = hex::encode(&protocol_header.proof_of_work_nonce());
        protocol_block_header_obj.insert(
            "proof_of_work_nonce".to_string(),
            serde_json::Value::String(proof_of_work_str),
        );
        protocol_block_header_obj.insert(
            "protocol".to_string(),
            serde_json::Value::String(H::NAME.to_string()),
        );

        for ops_list in &mut operations {
            for op in ops_list {
                op.hash = None;
                for content in &mut op.contents {
                    if let Some(content_obj) = content.as_object_mut() {
                        content_obj.remove("metadata");
                    }
                }
            }
        }
        let block_data = BlockData {
            protocol_data,
            operations,
        };

        let s = format!("chains/main/blocks/{predecessor_hash}/helpers/preapply/block");
        let mut url = self.endpoint.join(&s).expect("valid constant url");
        url.query_pairs_mut()
            .append_pair("timestamp", &timestamp.to_string());
        let body = serde_json::to_string(&block_data)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        let PreapplyResponse {
            shell_header,
            operations,
        } = self
            .single_response_blocking(&url, Some(body.clone()), None)
            .map_err(|inner| match inner {
                RpcError::WithContext { url, inner } => RpcError::WithBody { url, body, inner },
                _ => unreachable!(),
            })?;

        let full_block_header =
            protocol_header
                .full_header(shell_header)
                .map_err(|inner| RpcError::WithContext {
                    url: url.clone(),
                    inner,
                })?;

        Ok((Box::new(full_block_header), operations))
    }

    pub fn inject_block(
        &self,
        data: String,
        operations: Vec<Vec<DecodedOperation>>,
    ) -> Result<BlockHash, RpcError> {
        #[derive(Serialize)]
        struct BlockData {
            data: String,
            operations: Vec<Vec<DecodedOperation>>,
        }
        let block_data = BlockData { data, operations };
        let url = self
            .endpoint
            .join("injection/block")
            .expect("valid constant url");
        let body = serde_json::to_string(&block_data)
            .map_err(Into::into)
            .map_err(RpcError::Less)?;
        self.single_response_blocking(&url, Some(body), None)
    }

    fn multiple_responses<T, F>(
        &self,
        url: &Url,
        timeout: Option<Duration>,
        wrapper: F,
    ) -> Result<(), RpcErrorInner>
    where
        T: DeserializeOwned,
        F: Fn(T) -> Result<BakerAction, RpcError> + Send + 'static,
    {
        let request = self.inner.get(url.clone());
        let request = if let Some(timeout) = timeout {
            request.timeout(timeout)
        } else {
            request
        };

        let url = url.clone();
        let response = request.send()?;
        let tx = self.tx.clone();
        let handle = thread::spawn(move || {
            let status = response.status();

            if status.is_success() {
                let mut deserializer =
                    serde_json::Deserializer::from_reader(response).into_iter::<T>();
                let wrapper = &wrapper;
                for v in deserializer.by_ref() {
                    let url = url.clone();
                    let v = v
                        .map_err(|err| RpcError::WithContext {
                            url,
                            inner: err.into(),
                        })
                        .and_then(wrapper);
                    let action = match v {
                        Ok(v) => v,
                        Err(err) => BakerAction::RpcError(RpcErrorAction {
                            error: err.to_string(),
                        }),
                    };
                    let _ = tx.send(action);
                }
            } else {
                let status = response.status();
                let mut response = response;
                match read_error(&mut response, status) {
                    Ok(()) => unreachable!(),
                    Err(inner) => {
                        let _ = tx.send(BakerAction::RpcError(RpcErrorAction {
                            error: RpcError::WithContext { url, inner }.to_string(),
                        }));
                    }
                }
            }
        });

        // TODO: join
        let _ = handle;

        Ok(())
    }

    fn single_response_blocking<T>(
        &self,
        url: &Url,
        body: Option<String>,
        timeout: Option<Duration>,
    ) -> Result<T, RpcError>
    where
        T: DeserializeOwned,
    {
        let mut retry = 3;
        let mut response = loop {
            let request = match &body {
                Some(ref body) => self.inner.post(url.clone()).body(body.clone()),
                None => self.inner.get(url.clone()),
            };
            let request = if let Some(timeout) = timeout {
                request.timeout(timeout)
            } else {
                request
            };
            match request.send() {
                Ok(v) => break v,
                Err(err) => {
                    if retry == 0 {
                        return Err(RpcError::WithContext {
                            url: url.clone(),
                            inner: err.into(),
                        });
                    } else {
                        retry -= 1;
                    }
                }
            }
        };
        if response.status().is_success() {
            serde_json::from_reader::<_, T>(response).map_err(|err| RpcError::WithContext {
                url: url.clone(),
                inner: err.into(),
            })
        } else {
            let status = response.status();
            read_error(&mut response, status).map_err(|err| RpcError::WithContext {
                url: url.clone(),
                inner: err,
            })?;
            unreachable!()
        }
    }
}

// it may be string without quotes, it is invalid json, let's read it manually
fn read_error(response: &mut impl io::Read, status: StatusCode) -> Result<(), RpcErrorInner> {
    let mut buf = [0; 0x1000];
    io::Read::read(response, &mut buf)?;
    let err = str::from_utf8(&buf)?
        .trim_end_matches('\0')
        .trim_end_matches('\n');
    Err(RpcErrorInner::NodeError(err.to_string(), status))
}

fn convert_timestamp(v: &str) -> Result<u64, RpcErrorInner> {
    OffsetDateTime::parse(v, &Rfc3339)
        .map_err(Into::into)
        .map(|v| v.unix_timestamp() as u64)
}

fn convert_fitness(f: &[String]) -> Result<i32, RpcErrorInner> {
    let round_bytes = hex::decode(f.get(4).ok_or(RpcErrorInner::InvalidFitness)?)?
        .as_slice()
        .try_into()
        .ok()
        .ok_or(RpcErrorInner::InvalidFitness)?;
    Ok(i32::from_be_bytes(round_bytes))
}
