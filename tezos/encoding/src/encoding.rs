// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Schema used for serialization and deserialization.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::str::FromStr;

use crypto::hash::HashType;

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum FieldName {
    A,                              // a
    Applied,                        // applied
    Arg,                            // arg
    B,                              // b
    BakingRewardPerEndorsement,     // baking_reward_per_endorsement
    BlockHeader,                    // block_header
    BlockHeaderProtoJson,           // block_header_proto_json
    BlockHeaderProtoMetadataJson,   // block_header_proto_metadata_json
    BlockReward,                    // block_reward
    BlockSecurityDeposit,           // block_security_deposit
    BlocksPerCommitment,            // blocks_per_commitment
    BlocksPerCycle,                 // blocks_per_cycle
    BlocksPerRollSnapshot,          // blocks_per_roll_snapshot
    BlocksPerVotingPeriod,          // blocks_per_voting_period
    Body,                           // body
    Branch,                         // branch
    BranchDelayed,                  // branch_delayed
    BranchRefused,                  // branch_refused
    C,                              // c
    ChainArg,                       // chain_arg
    ChainID,                        // chain_id
    ChainName,                      // chain_name
    Components,                     // components
    Context,                        // context
    ContextHash,                    // context_hash
    ContextPath,                    // context_path
    CostPerByte,                    // cost_per_byte
    Counter,                        // counter
    CurrentBlockHeader,             // current_block_header
    CurrentBranch,                  // current_branch
    CurrentHead,                    // current_head
    CurrentMempool,                 // current_mempool
    Data,                           // data
    D,                              // d
    Deactivate,                     // deactivate
    DelayPerMissingEndorsement,     // delay_per_missing_endorsement
    DisableMempool,                 // disable_mempool
    DistributedDbVersion,           // distributed_db_version
    E,                              // e
    EndorsementReward,              // endorsement_reward
    EndorsementSecurityDeposit,     // endorsement_security_deposit
    EndorsersPerBlock,              // endorsers_per_block
    ErrorJson,                      // error_json
    ExpectedEnvVersion,             // expected_env_version
    F,                              // f
    FFIService,                     // ffi_service
    Fitness,                        // fitness
    ForkingBlockHash,               // forking_block_hash
    ForkingTestchain,               // forking_testchain
    GetBlockHeaders,                // get_block_headers
    GetOperationHashesForBlocks,    // get_operation_hashes_for_blocks
    GetOperationsForBlocks,         // get_operations_for_blocks
    GetOperations,                  // get_operations
    GetProtocols,                   // get_protocols
    HardGasLimitPerBlock,           // hard_gas_limit_per_block
    HardGasLimitPerOperation,       // hard_gas_limit_per_operation
    HardStorageLimitPerOperation,   // hard_storage_limit_per_operation
    Hash,                           // hash
    H,                              // h
    History,                        // history
    ID,                             // id
    Implementation,                 // implementation
    InitialEndorsers,               // initial_endorsers
    Interface,                      // interface
    IsEndorsement,                  // is_endorsement
    KnownValid,                     // known_valid
    LastAllowedForkLevel,           // last_allowed_fork_level
    Left,                           // left
    Level,                          // level
    Major,                          // major
    MaxOperationsTTL,               // max_operations_ttl
    MessageNonce,                   // message_nonce
    Messages,                       // messages
    MichelsonMaximumTypeSize,       // michelson_maximum_type_size
    Minor,                          // minor
    MinProposalQuorum,              // min_proposal_quorum
    Motive,                         // motive
    Name,                           // name
    Ofs,                            // ofs
    OperationHashesForBlock,        // operation_hashes_for_block
    OperationHashes,                // operation_hashes
    OperationHashesPath,            // operation_hashes_path
    Operation,                      // operation
    OperationsHash,                 // operations_hash
    Operations,                     // operations
    OperationsForBlock,             // operations_for_block
    OperationsProtoMetadataJson,    // operations_proto_metadata_json
    OriginationBurn,                // origination_burn
    OriginationSize,                // origination_size
    P2PVersion,                     // p2p_version
    Path,                           // path
    PeerID,                         // peer_id
    Pending,                        // pending
    Point,                          // point
    Port,                           // port
    PotentialPeersToConnect,        // potential_peers_to_connect
    P,                              // p
    Predecessor,                    // predecessor
    PredHeader,                     // pred_header
    PreservedCycles,                // preserved_cycles
    Prevalidator,                   // prevalidator
    PrivateNode,                    // private_node
    ProofOfWorkStamp,               // proof_of_work_stamp
    ProofOfWorkThreshold,           // proof_of_work_threshold
    ProtocolDataJson,               // protocol_data_json
    ProtocolDataJsonWithErrorJson,  // protocol_data_json_with_error_json
    ProtocolData,                   // protocol_data
    Protocol,                       // protocol
    Proto,                          // proto
    PublicKey,                      // public_key
    QuorumMax,                      // quorum_max
    QuorumMin,                      // quorum_min
    Refused,                        // refused
    Request,                        // request
    Result,                         // result
    Right,                          // right
    SeedNonceRevelationTip,         // seed_nonce_revelation_tip
    S,                              // s
    TestChainDuration,              // test_chain_duration
    TestChainID,                    // test_chain_id
    TimeBetweenBlocks,              // time_between_blocks
    Timestamp,                      // timestamp
    TokensPerRoll,                  // tokens_per_roll
    T,                              // t
    ValidationPass,                 // validation_pass
    ValidationResultMessage,        // validation_result_message
    Versions,                       // versions
    V,                              // v
    X,                              // x
    Y,                              // y
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FieldName {
    pub fn as_str(&self) -> &'static str {
        match *self {
            FieldName::A => "a",
            FieldName::Applied => "applied",
            FieldName::Arg => "arg",
            FieldName::B => "b",
            FieldName::BakingRewardPerEndorsement => "baking_reward_per_endorsement",
            FieldName::BlockHeader => "block_header",
            FieldName::BlockHeaderProtoJson => "block_header_proto_json",
            FieldName::BlockHeaderProtoMetadataJson => "block_header_proto_metadata_json",
            FieldName::BlockReward => "block_reward",
            FieldName::BlockSecurityDeposit => "block_security_deposit",
            FieldName::BlocksPerCommitment => "blocks_per_commitment",
            FieldName::BlocksPerCycle => "blocks_per_cycle",
            FieldName::BlocksPerRollSnapshot => "blocks_per_roll_snapshot",
            FieldName::BlocksPerVotingPeriod => "blocks_per_voting_period",
            FieldName::Body => "body",
            FieldName::Branch => "branch",
            FieldName::BranchDelayed => "branch_delayed",
            FieldName::BranchRefused => "branch_refused",
            FieldName::C => "c",
            FieldName::ChainArg => "chain_arg",
            FieldName::ChainID => "chain_id",
            FieldName::ChainName => "chain_name",
            FieldName::Components => "components",
            FieldName::Context => "context",
            FieldName::ContextHash => "context_hash",
            FieldName::ContextPath => "context_path",
            FieldName::CostPerByte => "cost_per_byte",
            FieldName::Counter => "counter",
            FieldName::CurrentBlockHeader => "current_block_header",
            FieldName::CurrentBranch => "current_branch",
            FieldName::CurrentHead => "current_head",
            FieldName::CurrentMempool => "current_mempool",
            FieldName::Data => "data",
            FieldName::D => "d",
            FieldName::Deactivate => "deactivate",
            FieldName::DelayPerMissingEndorsement => "delay_per_missing_endorsement",
            FieldName::DisableMempool => "disable_mempool",
            FieldName::DistributedDbVersion => "distributed_db_version",
            FieldName::E => "e",
            FieldName::EndorsementReward => "endorsement_reward",
            FieldName::EndorsementSecurityDeposit => "endorsement_security_deposit",
            FieldName::EndorsersPerBlock => "endorsers_per_block",
            FieldName::ErrorJson => "error_json",
            FieldName::ExpectedEnvVersion => "expected_env_version",
            FieldName::F => "f",
            FieldName::FFIService => "ffi_service",
            FieldName::Fitness => "fitness",
            FieldName::ForkingBlockHash => "forking_block_hash",
            FieldName::ForkingTestchain => "forking_testchain",
            FieldName::GetBlockHeaders => "get_block_headers",
            FieldName::GetOperationHashesForBlocks => "get_operation_hashes_for_blocks",
            FieldName::GetOperationsForBlocks => "get_operations_for_blocks",
            FieldName::GetOperations => "get_operations",
            FieldName::GetProtocols => "get_protocols",
            FieldName::HardGasLimitPerBlock => "hard_gas_limit_per_block",
            FieldName::HardGasLimitPerOperation => "hard_gas_limit_per_operation",
            FieldName::HardStorageLimitPerOperation => "hard_storage_limit_per_operation",
            FieldName::Hash => "hash",
            FieldName::H => "h",
            FieldName::History => "history",
            FieldName::ID => "id",
            FieldName::Implementation => "implementation",
            FieldName::InitialEndorsers => "initial_endorsers",
            FieldName::Interface => "interface",
            FieldName::IsEndorsement => "is_endorsement",
            FieldName::KnownValid => "known_valid",
            FieldName::LastAllowedForkLevel => "last_allowed_fork_level",
            FieldName::Left => "left",
            FieldName::Level => "level",
            FieldName::Major => "major",
            FieldName::MaxOperationsTTL => "max_operations_ttl",
            FieldName::MessageNonce => "message_nonce",
            FieldName::Messages => "messages",
            FieldName::MichelsonMaximumTypeSize => "michelson_maximum_type_size",
            FieldName::Minor => "minor",
            FieldName::MinProposalQuorum => "min_proposal_quorum",
            FieldName::Motive => "motive",
            FieldName::Name => "name",
            FieldName::Ofs => "ofs",
            FieldName::OperationHashesForBlock => "operation_hashes_for_block",
            FieldName::OperationHashes => "operation_hashes",
            FieldName::OperationHashesPath => "operation_hashes_path",
            FieldName::Operation => "operation",
            FieldName::OperationsHash => "operations_hash",
            FieldName::Operations => "operations",
            FieldName::OperationsForBlock => "operations_for_block",
            FieldName::OperationsProtoMetadataJson => "operations_proto_metadata_json",
            FieldName::OriginationBurn => "origination_burn",
            FieldName::OriginationSize => "origination_size",
            FieldName::P2PVersion => "p2p_version",
            FieldName::Path => "path",
            FieldName::PeerID => "peer_id",
            FieldName::Pending => "pending",
            FieldName::Point => "point",
            FieldName::Port => "port",
            FieldName::PotentialPeersToConnect => "potential_peers_to_connect",
            FieldName::P => "p",
            FieldName::Predecessor => "predecessor",
            FieldName::PredHeader => "pred_header",
            FieldName::PreservedCycles => "preserved_cycles",
            FieldName::Prevalidator => "prevalidator",
            FieldName::PrivateNode => "private_node",
            FieldName::ProofOfWorkStamp => "proof_of_work_stamp",
            FieldName::ProofOfWorkThreshold => "proof_of_work_threshold",
            FieldName::ProtocolDataJson => "protocol_data_json",
            FieldName::ProtocolDataJsonWithErrorJson => "protocol_data_json_with_error_json",
            FieldName::ProtocolData => "protocol_data",
            FieldName::Protocol => "protocol",
            FieldName::Proto => "proto",
            FieldName::PublicKey => "public_key",
            FieldName::QuorumMax => "quorum_max",
            FieldName::QuorumMin => "quorum_min",
            FieldName::Refused => "refused",
            FieldName::Request => "request",
            FieldName::Result => "result",
            FieldName::Right => "right",
            FieldName::SeedNonceRevelationTip => "seed_nonce_revelation_tip",
            FieldName::S => "s",
            FieldName::TestChainDuration => "test_chain_duration",
            FieldName::TestChainID => "test_chain_id",
            FieldName::TimeBetweenBlocks => "time_between_blocks",
            FieldName::Timestamp => "timestamp",
            FieldName::TokensPerRoll => "tokens_per_roll",
            FieldName::T => "t",
            FieldName::ValidationPass => "validation_pass",
            FieldName::ValidationResultMessage => "validation_result_message",
            FieldName::Versions => "versions",
            FieldName::V => "v",
            FieldName::X => "x",
            FieldName::Y => "y",
        }
    }
}

impl FromStr for FieldName {
    type Err = ();

    fn from_str(input: &str) -> Result<FieldName, Self::Err> {
        match input {
            "a" => Ok(FieldName::A),
            "applied" => Ok(FieldName::Applied),
            "arg" => Ok(FieldName::Arg),
            "b" => Ok(FieldName::B),
            "baking_reward_per_endorsement" => Ok(FieldName::BakingRewardPerEndorsement),
            "block_header" => Ok(FieldName::BlockHeader),
            "block_header_proto_json" => Ok(FieldName::BlockHeaderProtoJson),
            "block_header_proto_metadata_json" => Ok(FieldName::BlockHeaderProtoMetadataJson),
            "block_reward" => Ok(FieldName::BlockReward),
            "block_security_deposit" => Ok(FieldName::BlockSecurityDeposit),
            "blocks_per_commitment" => Ok(FieldName::BlocksPerCommitment),
            "blocks_per_cycle" => Ok(FieldName::BlocksPerCycle),
            "blocks_per_roll_snapshot" => Ok(FieldName::BlocksPerRollSnapshot),
            "blocks_per_voting_period" => Ok(FieldName::BlocksPerVotingPeriod),
            "body" => Ok(FieldName::Body),
            "branch" => Ok(FieldName::Branch),
            "branch_delayed" => Ok(FieldName::BranchDelayed),
            "branch_refused" => Ok(FieldName::BranchRefused),
            "c" => Ok(FieldName::C),
            "chain_arg" => Ok(FieldName::ChainArg),
            "chain_id" => Ok(FieldName::ChainID),
            "chain_name" => Ok(FieldName::ChainName),
            "components" => Ok(FieldName::Components),
            "context" => Ok(FieldName::Context),
            "context_hash" => Ok(FieldName::ContextHash),
            "context_path" => Ok(FieldName::ContextPath),
            "cost_per_byte" => Ok(FieldName::CostPerByte),
            "counter" => Ok(FieldName::Counter),
            "current_block_header" => Ok(FieldName::CurrentBlockHeader),
            "current_branch" => Ok(FieldName::CurrentBranch),
            "current_head" => Ok(FieldName::CurrentHead),
            "current_mempool" => Ok(FieldName::CurrentMempool),
            "data" => Ok(FieldName::Data),
            "d" => Ok(FieldName::D),
            "deactivate" => Ok(FieldName::Deactivate),
            "delay_per_missing_endorsement" => Ok(FieldName::DelayPerMissingEndorsement),
            "disable_mempool" => Ok(FieldName::DisableMempool),
            "distributed_db_version" => Ok(FieldName::DistributedDbVersion),
            "e" => Ok(FieldName::E),
            "endorsement_reward" => Ok(FieldName::EndorsementReward),
            "endorsement_security_deposit" => Ok(FieldName::EndorsementSecurityDeposit),
            "endorsers_per_block" => Ok(FieldName::EndorsersPerBlock),
            "error_json" => Ok(FieldName::ErrorJson),
            "expected_env_version" => Ok(FieldName::ExpectedEnvVersion),
            "f" => Ok(FieldName::F),
            "ffi_service" => Ok(FieldName::FFIService),
            "fitness" => Ok(FieldName::Fitness),
            "forking_block_hash" => Ok(FieldName::ForkingBlockHash),
            "forking_testchain" => Ok(FieldName::ForkingTestchain),
            "get_block_headers" => Ok(FieldName::GetBlockHeaders),
            "get_operation_hashes_for_blocks" => Ok(FieldName::GetOperationHashesForBlocks),
            "get_operations_for_blocks" => Ok(FieldName::GetOperationsForBlocks),
            "get_operations" => Ok(FieldName::GetOperations),
            "get_protocols" => Ok(FieldName::GetProtocols),
            "hard_gas_limit_per_block" => Ok(FieldName::HardGasLimitPerBlock),
            "hard_gas_limit_per_operation" => Ok(FieldName::HardGasLimitPerOperation),
            "hard_storage_limit_per_operation" => Ok(FieldName::HardStorageLimitPerOperation),
            "hash" => Ok(FieldName::Hash),
            "h" => Ok(FieldName::H),
            "history" => Ok(FieldName::History),
            "id" => Ok(FieldName::ID),
            "implementation" => Ok(FieldName::Implementation),
            "initial_endorsers" => Ok(FieldName::InitialEndorsers),
            "interface" => Ok(FieldName::Interface),
            "is_endorsement" => Ok(FieldName::IsEndorsement),
            "known_valid" => Ok(FieldName::KnownValid),
            "last_allowed_fork_level" => Ok(FieldName::LastAllowedForkLevel),
            "left" => Ok(FieldName::Left),
            "level" => Ok(FieldName::Level),
            "major" => Ok(FieldName::Major),
            "max_operations_ttl" => Ok(FieldName::MaxOperationsTTL),
            "message_nonce" => Ok(FieldName::MessageNonce),
            "messages" => Ok(FieldName::Messages),
            "michelson_maximum_type_size" => Ok(FieldName::MichelsonMaximumTypeSize),
            "minor" => Ok(FieldName::Minor),
            "min_proposal_quorum" => Ok(FieldName::MinProposalQuorum),
            "motive" => Ok(FieldName::Motive),
            "name" => Ok(FieldName::Name),
            "ofs" => Ok(FieldName::Ofs),
            "operation_hashes_for_block" => Ok(FieldName::OperationHashesForBlock),
            "operation_hashes" => Ok(FieldName::OperationHashes),
            "operation_hashes_path" => Ok(FieldName::OperationHashesPath),
            "operation" => Ok(FieldName::Operation),
            "operations_hash" => Ok(FieldName::OperationsHash),
            "operations" => Ok(FieldName::Operations),
            "operations_for_block" => Ok(FieldName::OperationsForBlock),
            "operations_proto_metadata_json" => Ok(FieldName::OperationsProtoMetadataJson),
            "origination_burn" => Ok(FieldName::OriginationBurn),
            "origination_size" => Ok(FieldName::OriginationSize),
            "p2p_version" => Ok(FieldName::P2PVersion),
            "path" => Ok(FieldName::Path),
            "peer_id" => Ok(FieldName::PeerID),
            "pending" => Ok(FieldName::Pending),
            "point" => Ok(FieldName::Point),
            "port" => Ok(FieldName::Port),
            "potential_peers_to_connect" => Ok(FieldName::PotentialPeersToConnect),
            "p" => Ok(FieldName::P),
            "predecessor" => Ok(FieldName::Predecessor),
            "pred_header" => Ok(FieldName::PredHeader),
            "preserved_cycles" => Ok(FieldName::PreservedCycles),
            "prevalidator" => Ok(FieldName::Prevalidator),
            "private_node" => Ok(FieldName::PrivateNode),
            "proof_of_work_stamp" => Ok(FieldName::ProofOfWorkStamp),
            "proof_of_work_threshold" => Ok(FieldName::ProofOfWorkThreshold),
            "protocol_data_json" => Ok(FieldName::ProtocolDataJson),
            "protocol_data_json_with_error_json" => Ok(FieldName::ProtocolDataJsonWithErrorJson),
            "protocol_data" => Ok(FieldName::ProtocolData),
            "protocol" => Ok(FieldName::Protocol),
            "proto" => Ok(FieldName::Proto),
            "public_key" => Ok(FieldName::PublicKey),
            "quorum_max" => Ok(FieldName::QuorumMax),
            "quorum_min" => Ok(FieldName::QuorumMin),
            "refused" => Ok(FieldName::Refused),
            "request" => Ok(FieldName::Request),
            "result" => Ok(FieldName::Result),
            "right" => Ok(FieldName::Right),
            "seed_nonce_revelation_tip" => Ok(FieldName::SeedNonceRevelationTip),
            "s" => Ok(FieldName::S),
            "test_chain_duration" => Ok(FieldName::TestChainDuration),
            "test_chain_id" => Ok(FieldName::TestChainID),
            "time_between_blocks" => Ok(FieldName::TimeBetweenBlocks),
            "timestamp" => Ok(FieldName::Timestamp),
            "tokens_per_roll" => Ok(FieldName::TokensPerRoll),
            "t" => Ok(FieldName::T),
            "validation_pass" => Ok(FieldName::ValidationPass),
            "validation_result_message" => Ok(FieldName::ValidationResultMessage),
            "versions" => Ok(FieldName::Versions),
            "v" => Ok(FieldName::V),
            "x" => Ok(FieldName::X),
            "y" => Ok(FieldName::Y),
            _      => panic!(input.to_owned()),//Err(()),
        }
    }
}


#[derive(Debug, Clone)]
pub struct Field {
    name: FieldName,
    encoding: Encoding,
}

impl Field {
    pub fn new(name: FieldName, encoding: Encoding) -> Field {
        Field { name, encoding }
    }

    pub fn get_name(&self) -> &FieldName {
        &self.name
    }

    pub fn get_encoding(&self) -> &Encoding {
        &self.encoding
    }
}

pub type Schema = Vec<Field>;

#[derive(Debug, Clone)]
pub struct Tag {
    id: u16,
    variant: String,
    encoding: Encoding,
}

impl Tag {
    pub fn new(id: u16, variant: &str, encoding: Encoding) -> Tag {
        Tag { id, variant: String::from(variant), encoding }
    }

    pub fn get_id(&self) -> u16 {
        self.id
    }

    pub fn get_encoding(&self) -> &Encoding {
        &self.encoding
    }

    pub fn get_variant(&self) -> &String {
        &self.variant
    }
}

#[derive(Debug, Clone)]
pub struct TagMap {
    id_to_tag: HashMap<u16, Tag>,
    variant_to_tag: HashMap<String, Tag>,
}

impl TagMap {
    pub fn new(tags: &[Tag]) -> TagMap {
        let mut id_to_tag = HashMap::new();
        let mut variant_to_tag = HashMap::new();

        for tag in tags {
            let prev_item = id_to_tag.insert(tag.get_id(), tag.clone());
            assert!(prev_item.is_none(), "Tag id: 0x{:X} is already present in TagMap", tag.get_id());
            variant_to_tag.insert(tag.get_variant().to_string(), tag.clone());
        }

        TagMap { id_to_tag, variant_to_tag }
    }

    pub fn find_by_id(&self, id: u16) -> Option<&Tag> {
        self.id_to_tag.get(&id)
    }

    pub fn find_by_variant(&self, variant: &str) -> Option<&Tag> {
        self.variant_to_tag.get(variant)
    }
}

pub enum SchemaType {
    Json,
    Binary,
}

pub trait SplitEncodingFn: Fn(SchemaType) -> Encoding + Send + Sync {}

impl<F> SplitEncodingFn for F where F: Fn(SchemaType) -> Encoding + Send + Sync {}

impl fmt::Debug for dyn SplitEncodingFn<Output=Encoding> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Fn(SchemaType) -> Encoding")
    }
}


pub trait RecursiveEncodingFn: Fn() -> Encoding + Send + Sync {}

impl<F> RecursiveEncodingFn for F where F: Fn() -> Encoding + Send + Sync {}

impl fmt::Debug for dyn RecursiveEncodingFn<Output=Encoding> + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Fn() -> Encoding")
    }
}

/// Represents schema used for encoding a data into a json or a binary form.
#[derive(Debug, Clone)]
pub enum Encoding {
    /// Encoded as nothing in binary or null in json
    Unit,
    /// Signed 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Int8,
    /// Unsigned 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Uint8,
    /// Signed 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Int16,
    /// Unsigned 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Uint16,
    /// Signed 31 bit integer, which corresponds to type int on 32-bit OCaml systems (data is encoded as a 32 bit int in binary and an integer in JSON).
    Int31,
    /// Signed 32 bit integer (data is encoded as a 32-bit int in binary and an integer in JSON).
    Int32,
    /// Unsigned 32 bit integer (data is encoded as a 32-bit int in binary and an integer in JSON).
    Uint32,
    /// Signed 64 bit integer (data is encoded as a 64-bit int in binary and a decimal string in JSON).
    Int64,
    /// Integer with bounds in a given range. Both bounds are inclusive.
    RangedInt,
    ///  Big number
    ///  In JSON, data is encoded as a decimal string.
    ///  In binary, data is encoded as a variable length sequence of
    ///  bytes, with a running unary size bit: the most significant bit of
    ///  each byte tells is this is the last byte in the sequence (0) or if
    /// there is more to read (1). The second most significant bit of the
    /// first byte is reserved for the sign (positive if zero). Binary_size and
    /// sign bits ignored, data is then the binary representation of the
    /// absolute value of the number in little-endian order.
    Z,
    /// Big number
    /// Almost identical to [Encoding::Z], but does not contain the sign bit in the second most
    /// significant bit of the first byte
    Mutez,
    /// Encoding of floating point number (encoded as a floating point number in JSON and a double in binary).
    Float,
    /// Float with bounds in a given range. Both bounds are inclusive.
    RangedFloat,
    /// Encoding of a boolean (data is encoded as a byte in binary and a boolean in JSON).
    Bool,
    /// Encoding of a string
    /// - encoded as a byte sequence in binary prefixed by the length
    /// of the string
    /// - encoded as a string in JSON.
    String,
    /// Encoding of arbitrary sized bytes (encoded via hex in JSON and directly as a sequence byte in binary).
    Bytes,
    /// Tag is prefixed by tag id and followed by encoded bytes
    /// First argument represents size of the tag marker in bytes.
    Tags(usize, TagMap),
    /// List combinator. It's behavior is similar to [Encoding::Greedy] encoding. Main distinction
    /// is that we are expecting list of items instead of a single item to be contained in binary data.
    /// - encoded as an array in JSON
    /// - encoded as the concatenation of all the element in binary
    List(Box<Encoding>),
    /// Encode enumeration via association list
    ///  - represented as a string in JSON and
    ///  - represented as an integer representing the element's position in the list in binary. The integer size depends on the list size.
    Enum,
    /// Combinator to make an optional value
    /// (represented as a 1-byte tag followed by the data (or nothing) in binary
    ///  and either the raw value or an empty object in JSON).
    Option(Box<Encoding>),
    /// TE-172 - combinator to make an optional field, this is not the same as Encoding::Option
    /// (req "arg" (Data_encoding.option string)) is not the same as (opt "arg" string))
    OptionalField(Box<Encoding>),
    /// Is the collection of fields.
    /// not prefixed by anything in binary, encoded as the concatenation of all the element in binary
    Obj(Schema),
    /// Heterogeneous collection of values.
    /// Similar to [Encoding::Obj], but schema can be any types, not just fields.
    /// Encoded as continuous binary representation.
    Tup(Vec<Encoding>),
    /// Is the collection of fields.
    /// prefixed its length in bytes (4 Bytes), encoded as the concatenation of all the element in binary
    Dynamic(Box<Encoding>),
    /// Represents fixed size block in binary encoding.
    Sized(usize, Box<Encoding>),
    /// Almost same as [Encoding::Dynamic] but without bytes size information prefix.
    /// It assumes that encoding passed as argument will process rest of the available data.
    Greedy(Box<Encoding>),
    /// Decode various types of hashes. Hash has it's own predefined length and prefix.
    /// This is controller by a hash implementation.
    Hash(HashType),
    /// Provides different encoding based on target data type.
    Split(Arc<dyn SplitEncodingFn<Output=Encoding> + Send + Sync>),
    /// Timestamp encoding.
    /// - encoded as RFC 3339 in json
    /// - encoded as [Encoding::Int64] in binary
    Timestamp,
    /// This is used to handle recursive encodings needed to encode tree structure.
    /// Encoding itself produces no output in binary or json.
    Lazy(Arc<dyn RecursiveEncodingFn<Output=Encoding> + Send + Sync>),
}

impl Encoding {
    #[inline]
    pub fn try_unwrap_option_encoding(&self) -> &Encoding {
        match self {
            Encoding::Option(encoding) => encoding,
            Encoding::OptionalField(encoding) => encoding,
            _ => panic!("This function can be called only on Encoding::Option or Encoding::List but it was called on {:?}", self)
        }
    }

    /// Utility function to construct [Encoding::List] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn list(encoding: Encoding) -> Encoding {
        Encoding::List(Box::new(encoding))
    }

    /// Utility function to construct [Encoding::Sized] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn sized(bytes_sz: usize, encoding: Encoding) -> Encoding {
        Encoding::Sized(bytes_sz, Box::new(encoding))
    }

    /// Utility function to construct [Encoding::Greedy] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn greedy(encoding: Encoding) -> Encoding {
        Encoding::Greedy(Box::new(encoding))
    }

    /// Utility function to construct [Encoding::Dynamic] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn dynamic(encoding: Encoding) -> Encoding {
        Encoding::Dynamic(Box::new(encoding))
    }

    /// Utility function to construct [Encoding::Option] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn option(encoding: Encoding) -> Encoding {
        Encoding::Option(Box::new(encoding))
    }

    /// Utility function to construct [Encoding::OptionalField] without the need
    /// to manually create new [Box].
    #[inline]
    pub fn option_field(encoding: Encoding) -> Encoding {
        Encoding::OptionalField(Box::new(encoding))
    }
}

/// Indicates that type has it's own ser/de schema.
pub trait HasEncoding {
    fn encoding() -> Encoding;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_split() {
        let split_encoding = Encoding::Split(Arc::new(|schema_type| {
            match schema_type {
                SchemaType::Json => Encoding::Uint16,
                SchemaType::Binary => Encoding::Float
            }
        }));

        if let Encoding::Split(inner_encoding) = split_encoding {
            match inner_encoding(SchemaType::Json) {
                Encoding::Uint16 => {}
                _ => panic!("Was expecting Encoding::Uint16")
            }
            match inner_encoding(SchemaType::Binary) {
                Encoding::Float => {}
                _ => panic!("Was expecting Encoding::Float")
            }
        } else {
            panic!("Was expecting Encoding::Split");
        }
    }
}