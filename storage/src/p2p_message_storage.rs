use std::sync::Arc;
use crate::persistent::{KeyValueStoreWithSchema, PersistentStorage, KeyValueSchema, Decoder, SchemaError, Encoder};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use serde::{Serialize, Deserialize};
use crate::persistent::sequence::{Sequences, SequenceGenerator};
use crate::{StorageError, IteratorMode, Direction};
use bytes::BufMut;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use crate::p2p_message_storage::rpc_message::P2PRpcMessage;

pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PMessageStorage> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageStorage {
    kv: Arc<P2PMessageStorageKV>,
    seq: Arc<SequenceGenerator>,
}

impl P2PMessageStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(),
            seq: persistent_storage.seq().generator("p2p_exp_msg_index_gen"),
        }
    }

    pub fn store_connection_message(&mut self, msg: &ConnectionMessage) -> Result<(), StorageError> {
        self.store_msg(&P2PMessage::ConnectionMessage(msg.clone()))
    }

    pub fn store_metadata_message(&mut self, msg: &MetadataMessage) -> Result<(), StorageError> {
        self.store_msg(&P2PMessage::Metadata(msg.clone()))
    }

    pub fn store_peer_message(&mut self, msgs: &Vec<PeerMessage>) -> Result<(), StorageError> {
        self.store_msg(&P2PMessage::P2PMessage(msgs.clone()))
    }

    pub fn get_range(&self, start: u64, count: u64) -> Result<Vec<P2PRpcMessage>, StorageError> {
        // let key = P2PMessageKey { index: start };
        let mut ret = Vec::with_capacity(count as usize);
        for index in start..start + count {
            let key = P2PMessageKey { index };
            match self.kv.get(&key) {
                Ok(Some(value)) => ret.push(value.into()),
                Ok(None) => println!("Out of bounds: {:?}", key),
                Err(err) => println!("Bad value: {:?}", err),
            }
        }
        Ok(ret)
    }

    fn store_msg(&mut self, val: &P2PMessage) -> Result<(), StorageError> {
        let index = self.seq.next()?;
        let key = P2PMessageKey { index };
        Ok(self.kv.put(&key, &val)?)
    }
}

impl KeyValueSchema for P2PMessageStorage {
    type Key = P2PMessageKey;
    type Value = P2PMessage;

    fn name() -> &'static str { "p2p_message_storage" }
}

#[derive(Debug, Clone)]
pub struct P2PMessageKey {
    // TODO: Add addresses for prefixing
    pub index: u64,
}

/// * bytes layout: `[index(8)]` TODO: Add remote address as prefix
impl Decoder for P2PMessageKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let mut value = [0u8; 8];
        for (x, y) in value.iter_mut().zip(bytes) {
            *x = *y;
        }
        let index = u64::from_be_bytes(value);
        Ok(Self {
            index
        })
    }
}

impl Encoder for P2PMessageKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        Ok(self.index.to_be_bytes().to_vec())
    }
}

/// Types of messages stored in database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum P2PMessage {
    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage(ConnectionMessage),

    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage(Vec<PeerMessage>),

    Metadata(MetadataMessage),
}

impl Decoder for P2PMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for P2PMessage {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

pub mod rpc_message {
    use tezos_messages::p2p::encoding::prelude::*;
    use crypto::hash::HashType;
    use failure::Fail;
    use std::net::IpAddr;
    use super::P2PMessage;
    use serde::Serialize;
    use tezos_messages::p2p::encoding::operation_hashes_for_blocks::OperationHashesForBlock;
    use crate::p2p_message_storage::rpc_message::P2PRpcMessage::P2pMessage;

    #[derive(Debug, Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    /// Types of messages sent by external RPC, directly maps to the StoreMessage, with different naming
    pub enum P2PRpcMessage {
        ConnectionMessage {
            message: MappedConnectionMessage,
        },
        P2pMessage {
            messages: Vec<MappedPeerMessage>,
        },

        Metadata(MetadataMessage),
    }

    impl From<P2PMessage> for P2PRpcMessage {
        fn from(value: P2PMessage) -> Self {
            match value {
                P2PMessage::ConnectionMessage(msg) => {
                    P2PRpcMessage::ConnectionMessage {
                        message: msg.into(),
                    }
                }
                P2PMessage::P2PMessage(msgs) => {
                    P2PRpcMessage::P2pMessage {
                        messages: msgs.into_iter().map(|x| MappedPeerMessage::from(x)).collect(),
                    }
                }
                P2PMessage::Metadata(msg) => {
                    P2PRpcMessage::Metadata(msg)
                }
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedConnectionMessage {
        pub port: u16,
        pub versions: Vec<Version>,
        pub public_key: String,
        pub proof_of_work_stamp: String,
        pub message_nonce: String,
    }

    impl From<ConnectionMessage> for MappedConnectionMessage {
        fn from(value: ConnectionMessage) -> Self {
            let ConnectionMessage { port, versions, public_key, proof_of_work_stamp, message_nonce, .. } = value;

            Self {
                port,
                versions,
                public_key: hex::encode(public_key),
                proof_of_work_stamp: hex::encode(proof_of_work_stamp),
                message_nonce: hex::encode(message_nonce),
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum MappedPeerMessage {
        Disconnect,
        Bootstrap,
        Advertise(AdvertiseMessage),
        SwapRequest(SwapMessage),
        SwapAck(SwapMessage),
        GetCurrentBranch(MappedGetCurrentBranchMessage),
        CurrentBranch(MappedCurrentBranchMessage),
        Deactivate(MappedDeactivateMessage),
        GetCurrentHead(MappedGetCurrentHeadMessage),
        CurrentHead(MappedCurrentHeadMessage),
        GetBlockHeaders(MappedGetBlockHeadersMessage),
        BlockHeader(MappedBlockHeaderMessage),
        GetOperations(MappedGetOperationsMessage),
        Operation(MappedOperationMessage),
        GetProtocols(MappedGetProtocolsMessage),
        Protocol(ProtocolMessage),
        GetOperationHashesForBlocks(MappedGetOperationHashesForBlocksMessage),
        OperationHashesForBlock(MappedOperationHashesForBlocksMessage),
        GetOperationsForBlocks(MappedGetOperationsForBlocksMessage),
        OperationsForBlocks(MappedOperationsForBlocksMessage),
        Dummy,
    }

    impl From<PeerMessage> for MappedPeerMessage {
        fn from(value: PeerMessage) -> Self {
            match value {
                PeerMessage::Disconnect => MappedPeerMessage::Disconnect,
                PeerMessage::Bootstrap => MappedPeerMessage::Bootstrap,
                PeerMessage::Advertise(msg) => MappedPeerMessage::Advertise(msg),
                PeerMessage::SwapRequest(msg) => MappedPeerMessage::SwapRequest(msg),
                PeerMessage::SwapAck(msg) => MappedPeerMessage::SwapAck(msg),
                PeerMessage::GetCurrentBranch(msg) => MappedPeerMessage::GetCurrentBranch(msg.into()),
                PeerMessage::CurrentBranch(msg) => MappedPeerMessage::CurrentBranch(msg.into()),
                PeerMessage::Deactivate(msg) => MappedPeerMessage::Deactivate(msg.into()),
                PeerMessage::GetCurrentHead(msg) => MappedPeerMessage::GetCurrentHead(msg.into()),
                PeerMessage::CurrentHead(msg) => MappedPeerMessage::CurrentHead(msg.into()),
                PeerMessage::GetBlockHeaders(msg) => MappedPeerMessage::GetBlockHeaders(msg.into()),
                PeerMessage::BlockHeader(msg) => MappedPeerMessage::BlockHeader(msg.into()),
                PeerMessage::GetOperations(msg) => MappedPeerMessage::GetOperations(msg.into()),
                PeerMessage::Operation(msg) => MappedPeerMessage::Operation(msg.into()),
                PeerMessage::GetProtocols(msg) => MappedPeerMessage::GetProtocols(msg.into()),
                PeerMessage::Protocol(msg) => MappedPeerMessage::Protocol(msg.into()),
                PeerMessage::GetOperationHashesForBlocks(msg) => MappedPeerMessage::GetOperationHashesForBlocks(msg.into()),
                PeerMessage::OperationHashesForBlock(msg) => MappedPeerMessage::OperationHashesForBlock(msg.into()),
                PeerMessage::GetOperationsForBlocks(msg) => MappedPeerMessage::GetOperationsForBlocks(msg.into()),
                PeerMessage::OperationsForBlocks(msg) => MappedPeerMessage::OperationsForBlocks(msg.into()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperationsForBlocksMessage {
        operations_for_block: MappedOperationsForBlock,
        operation_hashes_path: Path,
        operations: Vec<MappedOperation>,
    }

    impl From<OperationsForBlocksMessage> for MappedOperationsForBlocksMessage {
        fn from(value: OperationsForBlocksMessage) -> Self {
            Self {
                operations_for_block: value.operations_for_block().into(),
                operation_hashes_path: value.operation_hashes_path().clone(),
                operations: value.operations().iter().map(|x| MappedOperation::from(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperation {
        branch: String,
        data: String,
    }

    impl From<&Operation> for MappedOperation {
        fn from(value: &Operation) -> Self {
            Self {
                branch: HashType::BlockHash.bytes_to_string(value.branch()),
                data: hex::encode(value.data()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetOperationsForBlocksMessage {
        get_operations_for_blocks: Vec<MappedOperationsForBlock>
    }

    impl From<GetOperationsForBlocksMessage> for MappedGetOperationsForBlocksMessage {
        fn from(value: GetOperationsForBlocksMessage) -> Self {
            Self {
                get_operations_for_blocks: value.get_operations_for_blocks().iter().map(|x| MappedOperationsForBlock::from(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperationsForBlock {
        hash: String,
        validation_pass: i8,
    }

    impl From<&OperationsForBlock> for MappedOperationsForBlock {
        fn from(value: &OperationsForBlock) -> Self {
            Self {
                hash: HashType::BlockHash.bytes_to_string(value.hash()),
                validation_pass: value.validation_pass().clone(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperationHashesForBlocksMessage {
        operation_hashes_for_block: MappedOperationHashesForBlock,
        operation_hashes_path: Path,
        operation_hashes: Vec<String>,
    }

    impl From<OperationHashesForBlocksMessage> for MappedOperationHashesForBlocksMessage {
        fn from(value: OperationHashesForBlocksMessage) -> Self {
            Self {
                operation_hashes_for_block: value.operation_hashes_for_block().into(),
                operation_hashes_path: value.operation_hashes_path().clone(),
                operation_hashes: value.operation_hashes().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetOperationHashesForBlocksMessage {
        get_operation_hashes_for_blocks: Vec<MappedOperationHashesForBlock>
    }

    impl From<GetOperationHashesForBlocksMessage> for MappedGetOperationHashesForBlocksMessage {
        fn from(value: GetOperationHashesForBlocksMessage) -> Self {
            Self {
                get_operation_hashes_for_blocks: value.get_operation_hashes_for_blocks().iter()
                    .map(|x| MappedOperationHashesForBlock::from(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperationHashesForBlock {
        hash: String,
        validation_pass: i8,
    }

    impl From<&OperationHashesForBlock> for MappedOperationHashesForBlock {
        fn from(value: &OperationHashesForBlock) -> Self {
            Self {
                hash: HashType::BlockHash.bytes_to_string(value.hash()),
                validation_pass: value.validation_pass(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetProtocolsMessage {
        get_protocols: Vec<String>,
    }

    impl From<GetProtocolsMessage> for MappedGetProtocolsMessage {
        fn from(value: GetProtocolsMessage) -> Self {
            let mut json = serde_json::to_value(value).unwrap();
            let protos: Vec<Vec<u8>> = serde_json::from_value(json.get_mut("get_protocols").unwrap().take()).unwrap();
            Self {
                get_protocols: protos.iter().map(|x| HashType::ProtocolHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedOperationMessage {
        branch: String,
        data: String,
    }

    impl From<OperationMessage> for MappedOperationMessage {
        fn from(value: OperationMessage) -> Self {
            let mut json = serde_json::to_value(value).unwrap();
            let operation: Operation = serde_json::from_value(json.get_mut("operation").unwrap().take()).unwrap();
            Self {
                branch: HashType::BlockHash.bytes_to_string(operation.branch()),
                data: hex::encode(operation.data()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetOperationsMessage {
        get_operations: Vec<String>
    }

    impl From<GetOperationsMessage> for MappedGetOperationsMessage {
        fn from(value: GetOperationsMessage) -> Self {
            let mut json = serde_json::to_value(value).unwrap();
            let ops: Vec<Vec<u8>> = serde_json::from_value(json.get_mut("get_operations").unwrap().take()).unwrap();
            Self {
                get_operations: ops.iter()
                    .map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedBlockHeaderMessage {
        block_header: MappedBlockHeader,
    }

    impl From<BlockHeaderMessage> for MappedBlockHeaderMessage {
        fn from(value: BlockHeaderMessage) -> Self {
            Self {
                block_header: value.block_header().clone().into()
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetBlockHeadersMessage {
        get_block_headers: Vec<String>
    }

    impl From<GetBlockHeadersMessage> for MappedGetBlockHeadersMessage {
        fn from(value: GetBlockHeadersMessage) -> Self {
            Self {
                get_block_headers: value.get_block_headers().iter().map(|x| HashType::BlockHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedCurrentHeadMessage {
        chain_id: String,
        current_block_header: MappedBlockHeader,
        current_mempool: MappedMempool,
    }

    impl From<CurrentHeadMessage> for MappedCurrentHeadMessage {
        fn from(value: CurrentHeadMessage) -> Self {
            Self {
                chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
                current_block_header: value.current_block_header().clone().into(),
                current_mempool: value.current_mempool().into(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedMempool {
        known_valid: Vec<String>,
        pending: Vec<String>,
    }

    impl From<&Mempool> for MappedMempool {
        fn from(value: &Mempool) -> Self {
            Self {
                known_valid: value.known_valid().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
                pending: value.pending().iter().map(|x| HashType::OperationHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetCurrentHeadMessage {
        chain_id: String
    }

    impl From<GetCurrentHeadMessage> for MappedGetCurrentHeadMessage {
        fn from(value: GetCurrentHeadMessage) -> Self {
            Self {
                chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedDeactivateMessage {
        deactivate: String,
    }

    impl From<DeactivateMessage> for MappedDeactivateMessage {
        fn from(value: DeactivateMessage) -> Self {
            Self {
                deactivate: HashType::ChainId.bytes_to_string(&value.deactivate()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedCurrentBranchMessage {
        chain_id: String,
        current_branch: MappedCurrentBranch,
    }

    impl From<CurrentBranchMessage> for MappedCurrentBranchMessage {
        fn from(value: CurrentBranchMessage) -> Self {
            Self {
                chain_id: HashType::ChainId.bytes_to_string(value.chain_id()),
                current_branch: value.current_branch().clone().into(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedCurrentBranch {
        current_head: MappedBlockHeader,
        history: Vec<String>,
    }

    impl From<CurrentBranch> for MappedCurrentBranch {
        fn from(value: CurrentBranch) -> Self {
            Self {
                current_head: value.current_head().clone().into(),
                history: value.history().iter().map(|x| HashType::BlockHash.bytes_to_string(x)).collect(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedBlockHeader {
        level: i32,
        proto: u8,
        predecessor: String,
        timestamp: i64,
        validation_pass: u8,
        operations_hash: String,
        fitness: Vec<Vec<u8>>,
        context: String,
        protocol_data: String,
    }

    impl From<BlockHeader> for MappedBlockHeader {
        fn from(value: BlockHeader) -> Self {
            Self {
                level: value.level().clone(),
                proto: value.proto().clone(),
                timestamp: value.timestamp().clone(),
                validation_pass: value.validation_pass().clone(),
                fitness: value.fitness().clone(),
                context: HashType::ContextHash.bytes_to_string(value.context()),
                operations_hash: HashType::OperationListListHash.bytes_to_string(value.operations_hash()),
                predecessor: HashType::BlockHash.bytes_to_string(value.predecessor()),
                protocol_data: hex::encode(value.protocol_data()),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MappedGetCurrentBranchMessage {
        pub chain_id: String
    }

    impl From<GetCurrentBranchMessage> for MappedGetCurrentBranchMessage {
        fn from(value: GetCurrentBranchMessage) -> Self {
            Self {
                chain_id: HashType::ChainId.bytes_to_string(&value.chain_id),
            }
        }
    }
}