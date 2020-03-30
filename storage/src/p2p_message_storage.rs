use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use crate::persistent::{KeyValueStoreWithSchema, PersistentStorage, KeyValueSchema, Decoder, SchemaError, Encoder};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use serde::{Serialize, Deserialize};
use crate::persistent::sequence::{Sequences, SequenceGenerator};
use crate::{StorageError, IteratorMode, Direction};
use bytes::BufMut;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;
use crate::p2p_message_storage::rpc_message::P2PRpcMessage;
use std::net::{SocketAddr, SocketAddrV4, Ipv4Addr, IpAddr, Ipv6Addr};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::p2p_message_storage::rpc_message::P2PRpcMessage::P2pMessage;
use lazy_static::lazy_static;
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

pub type P2PMessageStorageKV = dyn KeyValueStoreWithSchema<P2PMessageStorage> + Sync + Send;

lazy_static! {
    static ref COUNT: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
}

#[derive(Clone)]
pub struct P2PMessageStorage {
    kv: Arc<P2PMessageStorageKV>,
    host_index: P2PMessageSecondaryIndex,
    seq: Arc<SequenceGenerator>,
}

fn get_ts() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
}

impl P2PMessageStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(),
            host_index: P2PMessageSecondaryIndex::new(persistent_storage),
            seq: persistent_storage.seq().generator("p2p_exp_msg_index_gen"),
        }
    }

    fn count(&self) -> u64 {
        COUNT.load(Ordering::SeqCst)
    }

    fn inc_count(&mut self) {
        COUNT.fetch_add(1, Ordering::SeqCst);
    }

    pub fn store_connection_message(&mut self, msg: &ConnectionMessage, incoming: bool, remote_addr: SocketAddr) -> Result<(), StorageError> {
        let index = self.seq.next()?;
        let val = P2PMessage::ConnectionMessage {
            incoming,
            remote_addr,
            id: index,
            timestamp: get_ts(),
            message: msg.clone(),
        };

        self.host_index.put(remote_addr, index);
        self.kv.put(&index, &val)?;
        Ok(self.inc_count())
    }

    pub fn store_metadata_message(&mut self, msg: &MetadataMessage, incoming: bool, remote_addr: SocketAddr) -> Result<(), StorageError> {
        let index = self.seq.next()?;
        let val = P2PMessage::Metadata {
            incoming,
            remote_addr,
            id: index,
            timestamp: get_ts(),
            message: msg.clone(),
        };

        self.host_index.put(remote_addr, index);
        self.kv.put(&index, &val)?;
        Ok(self.inc_count())
    }

    pub fn store_peer_message(&mut self, msgs: &Vec<PeerMessage>, incoming: bool, remote_addr: SocketAddr) -> Result<(), StorageError> {
        let index = self.seq.next()?;
        let val = P2PMessage::P2PMessage {
            incoming,
            remote_addr,
            id: index,
            timestamp: get_ts(),
            message: msgs.clone(),
        };

        self.host_index.put(remote_addr, index);
        self.kv.put(&index, &val)?;
        Ok(self.inc_count())
    }

    pub fn get_range(&self, offset: u64, count: u64) -> Result<Vec<P2PRpcMessage>, StorageError> {
        let mut ret = Vec::with_capacity(count as usize);
        let end: u64 = self.count();
        let start = end.saturating_sub(offset).saturating_sub(count);
        for index in (start..start + count).rev() {
            match self.kv.get(&index) {
                Ok(Some(value)) => ret.push(value.into()),
                _ => continue,
            }
        }
        Ok(ret)
    }

    pub fn get_range_for_host(&self, host: SocketAddr, offset: u64, count: u64) -> Result<Vec<P2PRpcMessage>, StorageError> {
        let idx = self.host_index.get_for_host(host, offset, count)?;
        let mut ret = Vec::with_capacity(idx.len());
        for index in idx.iter().rev() {
            match self.kv.get(index) {
                Ok(Some(value)) => ret.push(value.into()),
                _ => continue,
            }
        }
        Ok(ret)
    }
}

impl KeyValueSchema for P2PMessageStorage {
    type Key = u64;
    type Value = P2PMessage;

    fn name() -> &'static str { "p2p_message_storage" }
}

pub type P2PMessageSecondaryIndexKV = dyn KeyValueStoreWithSchema<P2PMessageSecondaryIndex> + Sync + Send;

#[derive(Clone)]
pub struct P2PMessageSecondaryIndex {
    kv: Arc<P2PMessageSecondaryIndexKV>,
}

impl P2PMessageSecondaryIndex {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self { kv: persistent_storage.kv() }
    }

    #[inline]
    pub fn put(&mut self, sock_addr: SocketAddr, index: u64) -> Result<(), StorageError> {
        let key = P2PMessageSecondaryKey::new(sock_addr, index);
        Ok(self.kv.put(&key, &index)?)
    }

    pub fn get(&self, key: &P2PMessageSecondaryKey) -> Result<Option<u64>, StorageError> {
        Ok(self.kv.get(key)?)
    }

    pub fn get_for_host(&self, sock_addr: SocketAddr, offset: u64, limit: u64) -> Result<Vec<u64>, StorageError> {
        let (offset, limit) = (offset as usize, limit as usize);
        let key = P2PMessageSecondaryKey::prefix(sock_addr);

        let mut ret = Vec::with_capacity(limit);
        for (_key, value) in self.kv.prefix_iterator(&key)?.skip(offset) {
            ret.push(value?);
            if ret.len() == limit { break; }
        }

        ret.sort();
        Ok(ret)
    }
}

impl KeyValueSchema for P2PMessageSecondaryIndex {
    type Key = P2PMessageSecondaryKey;
    type Value = u64;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(16 + 2));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "p2p_message_secondary_index"
    }
}

#[derive(Debug, Clone)]
pub struct P2PMessageSecondaryKey {
    pub addr: u128,
    pub port: u16,
    pub index: u64,
}

impl P2PMessageSecondaryKey {
    pub fn new(sock_addr: SocketAddr, index: u64) -> Self {
        let addr = sock_addr.ip();
        let port = sock_addr.port();
        Self {
            addr: encode_address(&addr),
            port,
            index,
        }
    }

    pub fn prefix(sock_addr: SocketAddr) -> Self {
        Self::new(sock_addr, 0)
    }
}

fn encode_address(addr: &IpAddr) -> u128 {
    match addr {
        &IpAddr::V6(addr) => u128::from(addr),
        &IpAddr::V4(addr) => u32::from(addr) as u128,
    }
}

fn decode_address(value: u128) -> IpAddr {
    if value & 0xFFFFFFFFFFFFFFFFFFFFFFFF00000000 == 0 {
        IpAddr::V4(Ipv4Addr::from(value as u32))
    } else {
        IpAddr::V6(Ipv6Addr::from(value))
    }
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Decoder for P2PMessageSecondaryKey {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() != 26 {
            return Err(SchemaError::DecodeError);
        }
        let addr_value = &bytes[0..16];
        let port_value = &bytes[16..16 + 2];
        let index_value = &bytes[16 + 2..];
        // index
        let mut index = [0u8; 8];
        for (x, y) in index.iter_mut().zip(index_value) {
            *x = *y;
        }
        let index = u64::from_be_bytes(index);
        // port
        let mut port = [0u8; 2];
        for (x, y) in port.iter_mut().zip(port_value) {
            *x = *y;
        }
        let port = u16::from_be_bytes(port);
        // addr
        let mut addr = [0u8; 16];
        for (x, y) in addr.iter_mut().zip(addr_value) {
            *x = *y;
        }
        let addr = u128::from_be_bytes(addr);

        Ok(Self {
            addr,
            port,
            index,
        })
    }
}

/// * bytes layout: `[address(16)][port(2)][index(8)]`
impl Encoder for P2PMessageSecondaryKey {
    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut buf = Vec::with_capacity(26);
        buf.extend_from_slice(&self.addr.to_be_bytes());
        buf.extend_from_slice(&self.port.to_be_bytes());
        buf.extend_from_slice(&self.index.to_be_bytes());

        if buf.len() != 26 {
            println!("{:?} - {:?}", self, buf);
            Err(SchemaError::EncodeError)
        } else {
            Ok(buf)
        }
    }
}

/// Types of messages stored in database
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum P2PMessage {
    /// Unencrypted message, which is part of tezos communication handshake
    ConnectionMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: ConnectionMessage,
    },

    /// Actual deciphered P2P message sent by some tezos node
    P2PMessage {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: Vec<PeerMessage>,
    },

    Metadata {
        incoming: bool,
        timestamp: u128,
        id: u64,
        remote_addr: SocketAddr,
        message: MetadataMessage,
    },
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
    use std::net::{IpAddr, SocketAddr};
    use super::P2PMessage;
    use serde::Serialize;
    use tezos_messages::p2p::encoding::operation_hashes_for_blocks::OperationHashesForBlock;
    use crate::p2p_message_storage::rpc_message::P2PRpcMessage::P2pMessage;

    #[derive(Debug, Serialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    /// Types of messages sent by external RPC, directly maps to the StoreMessage, with different naming
    pub enum P2PRpcMessage {
        ConnectionMessage {
            incoming: bool,
            timestamp: u128,
            id: u64,
            remote_addr: SocketAddr,
            message: MappedConnectionMessage,
        },
        P2pMessage {
            incoming: bool,
            timestamp: u128,
            id: u64,
            remote_addr: SocketAddr,
            message: Vec<MappedPeerMessage>,
        },

        Metadata {
            incoming: bool,
            timestamp: u128,
            id: u64,
            remote_addr: SocketAddr,
            message: MetadataMessage,
        },
    }

    impl From<P2PMessage> for P2PRpcMessage {
        fn from(value: P2PMessage) -> Self {
            match value {
                P2PMessage::ConnectionMessage { incoming, timestamp, id, remote_addr, message } => {
                    P2PRpcMessage::ConnectionMessage {
                        incoming,
                        timestamp,
                        id,
                        remote_addr,
                        message: message.into(),
                    }
                }
                P2PMessage::P2PMessage { incoming, timestamp, id, remote_addr, message } => {
                    P2PRpcMessage::P2pMessage {
                        incoming,
                        timestamp,
                        id,
                        remote_addr,
                        message: message.into_iter().map(|x| MappedPeerMessage::from(x)).collect(),
                    }
                }
                P2PMessage::Metadata { incoming, timestamp, id, remote_addr, message } => {
                    P2PRpcMessage::Metadata {
                        incoming,
                        timestamp,
                        id,
                        remote_addr,
                        message,
                    }
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