// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::OperationHash;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use crate::p2p::binary_message::{MessageHash, SizeFromChunk};
use crate::p2p::encoding::prelude::*;
use crate::p2p::peer_message_size;

use super::limits::MESSAGE_MAX_SIZE;

pub const MESSAGE_TYPE_COUNT: usize = 18;

pub const MESSAGE_TYPE_TEXTS: [&str; MESSAGE_TYPE_COUNT] = [
    "Disconnect",
    "Advertise",
    "SwapRequest",
    "SwapAck",
    "Bootstrap",
    "GetCurrentBranch",
    "CurrentBranch",
    "Deactivate",
    "GetCurrentHead",
    "CurrentHead",
    "GetBlockHeaders",
    "BlockHeader",
    "GetOperations",
    "Operation",
    "GetProtocols",
    "Protocol",
    "GetOperationsForBlocks",
    "OperationsForBlocks",
];

#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader, BinWriter)]
#[encoding(tags = "u16")]
pub enum PeerMessage {
    #[encoding(tag = 0x01)]
    Disconnect,
    #[encoding(tag = 0x03)]
    Advertise(AdvertiseMessage),
    #[encoding(tag = 0x04)]
    SwapRequest(SwapMessage),
    #[encoding(tag = 0x05)]
    SwapAck(SwapMessage),
    #[encoding(tag = 0x02)]
    Bootstrap,
    #[encoding(tag = 0x10)]
    GetCurrentBranch(GetCurrentBranchMessage),
    #[encoding(tag = 0x11)]
    CurrentBranch(CurrentBranchMessage),
    #[encoding(tag = 0x12)]
    Deactivate(DeactivateMessage),
    #[encoding(tag = 0x13)]
    GetCurrentHead(GetCurrentHeadMessage),
    #[encoding(tag = 0x14)]
    CurrentHead(CurrentHeadMessage),
    #[encoding(tag = 0x20)]
    GetBlockHeaders(GetBlockHeadersMessage),
    #[encoding(tag = 0x21)]
    BlockHeader(BlockHeaderMessage),
    #[encoding(tag = 0x30)]
    GetOperations(GetOperationsMessage),
    #[encoding(tag = 0x31)]
    Operation(OperationMessage),
    #[encoding(tag = 0x40)]
    GetProtocols(GetProtocolsMessage),
    #[encoding(tag = 0x41)]
    Protocol(ProtocolMessage),
    #[encoding(tag = 0x60)]
    GetOperationsForBlocks(GetOperationsForBlocksMessage),
    #[encoding(tag = 0x61)]
    OperationsForBlocks(OperationsForBlocksMessage),
}

impl PeerMessage {
    pub fn type_str_for_message_index(index: usize) -> &'static str {
        if index < MESSAGE_TYPE_COUNT {
            MESSAGE_TYPE_TEXTS[index]
        } else {
            "<invalid index for message type>"
        }
    }

    pub fn index(&self) -> usize {
        match self {
            PeerMessage::Disconnect => 0,
            PeerMessage::Advertise(_) => 1,
            PeerMessage::SwapRequest(_) => 2,
            PeerMessage::SwapAck(_) => 3,
            PeerMessage::Bootstrap => 4,
            PeerMessage::GetCurrentBranch(_) => 5,
            PeerMessage::CurrentBranch(_) => 6,
            PeerMessage::Deactivate(_) => 7,
            PeerMessage::GetCurrentHead(_) => 8,
            PeerMessage::CurrentHead(_) => 9,
            PeerMessage::GetBlockHeaders(_) => 10,
            PeerMessage::BlockHeader(_) => 11,
            PeerMessage::GetOperations(_) => 12,
            PeerMessage::Operation(_) => 13,
            PeerMessage::GetProtocols(_) => 14,
            PeerMessage::Protocol(_) => 15,
            PeerMessage::GetOperationsForBlocks(_) => 16,
            PeerMessage::OperationsForBlocks(_) => 17,
        }
    }

    pub fn get_type_str(&self) -> &'static str {
        Self::type_str_for_message_index(self.index())
    }
}

impl slog::Value for PeerMessage {
    fn serialize(
        &self,
        _: &slog::Record,
        _: slog::Key,
        s: &mut dyn slog::Serializer,
    ) -> slog::Result {
        s.emit_arguments(
            "peer_message_kind",
            &format_args!("{}", self.get_type_str()),
        )?;
        match self {
            PeerMessage::Disconnect => Ok(()),
            PeerMessage::Advertise(v) => s.emit_arguments(
                "peer_message_advertise_addrs",
                &format_args!("{:?}", v.id()),
            ),
            PeerMessage::SwapRequest(v) => {
                s.emit_arguments(
                    "peer_message_swap_request_point",
                    &format_args!("{}", v.point()),
                )?;
                s.emit_arguments(
                    "peer_message_swap_request_peer_id",
                    &format_args!("{}", v.peer_id().to_base58_check()),
                )
            }
            PeerMessage::SwapAck(v) => {
                s.emit_arguments(
                    "peer_message_swap_ack_point",
                    &format_args!("{}", v.point()),
                )?;
                s.emit_arguments(
                    "peer_message_swap_ack_peer_id",
                    &format_args!("{}", v.peer_id().to_base58_check()),
                )
            }
            PeerMessage::Bootstrap => Ok(()),
            PeerMessage::GetCurrentBranch(v) => s.emit_arguments(
                "peer_message_get_current_branch_chain_id",
                &format_args!("{}", v.chain_id.to_base58_check()),
            ),
            PeerMessage::CurrentBranch(v) => {
                s.emit_arguments(
                    "peer_message_current_branch_current_head",
                    &format_args!("{:?}", v.current_branch().current_head()),
                )?;
                s.emit_arguments(
                    "peer_message_current_branch_history",
                    &format_args!("{:?}", v.current_branch().history()),
                )
            }
            PeerMessage::Deactivate(_) => Ok(()),
            PeerMessage::GetCurrentHead(v) => s.emit_arguments(
                "peer_message_get_current_head",
                &format_args!("{}", v.chain_id().to_base58_check()),
            ),
            PeerMessage::CurrentHead(v) => {
                s.emit_arguments(
                    "peer_message_current_head_chain_id",
                    &format_args!("{}", v.chain_id().to_base58_check()),
                )?;
                s.emit_arguments(
                    "peer_message_current_head_current_block_header",
                    &format_args!("{:?}", v.current_block_header()),
                )?;
                s.emit_arguments(
                    "peer_message_current_head_current_mempool",
                    &format_args!("{:?}", v.current_mempool()),
                )
            }
            PeerMessage::GetBlockHeaders(v) => s.emit_arguments(
                "peer_message_get_block_headers_hashes",
                &format_args!("{:?}", v.get_block_headers()),
            ),
            PeerMessage::BlockHeader(v) => {
                s.emit_arguments("peer_message_block_header", &format_args!("{:?}", v))
            }
            PeerMessage::GetOperations(v) => s.emit_arguments(
                "peer_message_get_operations_hashes",
                &format_args!("{:?}", v.get_operations()),
            ),
            PeerMessage::Operation(v) => {
                let op_hash_result = v.operation().message_typed_hash::<OperationHash>();
                s.emit_arguments(
                    "peer_message_operation_branch",
                    &format_args!("{:?}", v.operation().branch()),
                )?;
                s.emit_arguments(
                    "peer_message_operation_data",
                    &format_args!("{}", hex::encode(v.operation().data())),
                )?;
                s.emit_arguments(
                    "peer_message_operation_hash",
                    &format_args!("{:?}", op_hash_result),
                )
            }
            PeerMessage::GetProtocols(v) => {
                s.emit_arguments("peer_message_get_protocols", &format_args!("{:?}", v))
            }
            PeerMessage::Protocol(v) => {
                s.emit_arguments("peer_message_protocol", &format_args!("{:?}", v))
            }
            PeerMessage::GetOperationsForBlocks(v) => s.emit_arguments(
                "peer_message_get_operations_for_blocks",
                &format_args!("{:?}", v.get_operations_for_blocks()),
            ),
            PeerMessage::OperationsForBlocks(v) => {
                let op_hashes = v
                    .operations()
                    .iter()
                    .map(|x| x.message_typed_hash::<OperationHash>())
                    .collect::<Vec<_>>();
                s.emit_arguments(
                    "peer_message_operations_for_blocks_operation_hashes",
                    &format_args!("{:?}", op_hashes),
                )?;
                s.emit_arguments(
                    "peer_message_operations_for_blocks",
                    &format_args!("{:?}", v),
                )
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, Getters, HasEncoding, NomReader, BinWriter)]
#[encoding(dynamic = "MESSAGE_MAX_SIZE")]
pub struct PeerMessageResponse {
    #[get = "pub"]
    pub message: PeerMessage,

    /// Encrypted bytes read from stream including chunk sizes.
    #[encoding(skip)]
    #[get = "pub"]
    size_hint: Option<usize>,
}

impl PeerMessageResponse {
    pub fn set_size_hint(&mut self, size: usize) {
        self.size_hint = Some(size);
    }
}

impl From<PeerMessage> for PeerMessageResponse {
    fn from(message: PeerMessage) -> Self {
        PeerMessageResponse {
            message,
            size_hint: None,
        }
    }
}

macro_rules! into_peer_message {
    ($m:ident,$v:ident) => {
        impl From<$m> for PeerMessageResponse {
            fn from(msg: $m) -> Self {
                PeerMessage::$v(msg).into()
            }
        }

        impl From<$m> for std::sync::Arc<PeerMessageResponse> {
            fn from(msg: $m) -> Self {
                std::sync::Arc::new(PeerMessage::$v(msg).into())
            }
        }

        impl From<$m> for PeerMessage {
            fn from(msg: $m) -> Self {
                PeerMessage::$v(msg)
            }
        }
    };
}

into_peer_message!(AdvertiseMessage, Advertise);
into_peer_message!(GetCurrentBranchMessage, GetCurrentBranch);
into_peer_message!(CurrentBranchMessage, CurrentBranch);
into_peer_message!(GetBlockHeadersMessage, GetBlockHeaders);
into_peer_message!(BlockHeaderMessage, BlockHeader);
into_peer_message!(GetCurrentHeadMessage, GetCurrentHead);
into_peer_message!(CurrentHeadMessage, CurrentHead);
into_peer_message!(GetOperationsForBlocksMessage, GetOperationsForBlocks);
into_peer_message!(OperationsForBlocksMessage, OperationsForBlocks);
into_peer_message!(GetProtocolsMessage, GetProtocols);
into_peer_message!(ProtocolMessage, Protocol);
into_peer_message!(GetOperationsMessage, GetOperations);
into_peer_message!(OperationMessage, Operation);

impl SizeFromChunk for PeerMessageResponse {
    fn size_from_chunk(
        bytes: impl AsRef<[u8]>,
    ) -> Result<usize, tezos_encoding::binary_reader::BinaryReaderError> {
        peer_message_size(bytes.as_ref()).map(|s| s + 4)
    }
}
