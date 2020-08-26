// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding, Tag, TagMap, TagVariant};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::prelude::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerMessage {
    Disconnect,
    Advertise(AdvertiseMessage),
    SwapRequest(SwapMessage),
    SwapAck(SwapMessage),
    Bootstrap,
    GetCurrentBranch(GetCurrentBranchMessage),
    CurrentBranch(CurrentBranchMessage),
    Deactivate(DeactivateMessage),
    GetCurrentHead(GetCurrentHeadMessage),
    CurrentHead(CurrentHeadMessage),
    GetBlockHeaders(GetBlockHeadersMessage),
    BlockHeader(BlockHeaderMessage),
    GetOperations(GetOperationsMessage),
    Operation(OperationMessage),
    GetProtocols(GetProtocolsMessage),
    Protocol(ProtocolMessage),
    GetOperationHashesForBlocks(GetOperationHashesForBlocksMessage),
    OperationHashesForBlock(OperationHashesForBlocksMessage),
    GetOperationsForBlocks(GetOperationsForBlocksMessage),
    OperationsForBlocks(OperationsForBlocksMessage),
}


#[derive(Serialize, Deserialize, Debug, Getters)]
pub struct PeerMessageResponse {
    #[get = "pub"]
    messages: Vec<PeerMessage>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PeerMessageResponse, body);
has_encoding!(PeerMessageResponse, PEER_MESSAGE_RESPONSE_ENCODING, {
    Encoding::Obj(vec![
        Field::new(FieldName::Messages, Encoding::dynamic(Encoding::list(
            Encoding::Tags(
                size_of::<u16>(),
                TagMap::new(&[
                    Tag::new(0x01, TagVariant::Disconnect, Encoding::Unit),
                    Tag::new(0x02, TagVariant::Bootstrap, Encoding::Unit),
                    Tag::new(0x03, TagVariant::Advertise, AdvertiseMessage::encoding().clone()),
                    Tag::new(0x04, TagVariant::SwapRequest, SwapMessage::encoding().clone()),
                    Tag::new(0x05, TagVariant::SwapAck, SwapMessage::encoding().clone()),
                    Tag::new(0x10, TagVariant::GetCurrentBranch, GetCurrentBranchMessage::encoding().clone()),
                    Tag::new(0x11, TagVariant::CurrentBranch, CurrentBranchMessage::encoding().clone()),
                    Tag::new(0x12, TagVariant::Deactivate, DeactivateMessage::encoding().clone()),
                    Tag::new(0x13, TagVariant::GetCurrentHead, GetCurrentHeadMessage::encoding().clone()),
                    Tag::new(0x14, TagVariant::CurrentHead, CurrentHeadMessage::encoding().clone()),
                    Tag::new(0x20, TagVariant::GetBlockHeaders, GetBlockHeadersMessage::encoding().clone()),
                    Tag::new(0x21, TagVariant::BlockHeader, BlockHeaderMessage::encoding().clone()),
                    Tag::new(0x30, TagVariant::GetOperations, GetOperationsMessage::encoding().clone()),
                    Tag::new(0x31, TagVariant::Operation, OperationMessage::encoding().clone()),
                    Tag::new(0x40, TagVariant::GetProtocols, GetProtocolsMessage::encoding().clone()),
                    Tag::new(0x41, TagVariant::Protocol, ProtocolMessage::encoding().clone()),
                    Tag::new(0x50, TagVariant::GetOperationHashesForBlocks, GetOperationHashesForBlocksMessage::encoding().clone()),
                    Tag::new(0x51, TagVariant::OperationHashesForBlocks, OperationHashesForBlocksMessage::encoding().clone()),
                    Tag::new(0x60, TagVariant::GetOperationsForBlocks, GetOperationsForBlocksMessage::encoding().clone()),
                    Tag::new(0x61, TagVariant::OperationsForBlocks, OperationsForBlocksMessage::encoding().clone()),
                ])
            )
        )))
    ])
});

impl From<PeerMessage> for PeerMessageResponse {
    fn from(peer_message: PeerMessage) -> Self {
        PeerMessageResponse { messages: vec![peer_message], body: Default::default() }
    }
}

macro_rules! into_peer_message {
    ($m:ident,$v:ident) => {
        impl From<$m> for PeerMessageResponse {
            fn from(msg: $m) -> Self {
                PeerMessage::$v(msg).into()
            }
        }

        impl From<$m> for PeerMessage {
            fn from(msg: $m) -> Self {
                PeerMessage::$v(msg)
            }
        }
    }
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
into_peer_message!(GetOperationsMessage, GetOperations);
into_peer_message!(OperationMessage, Operation);
