// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;

use getset::Getters;
use serde::{Deserialize, Serialize};

use lazy_static::lazy_static;
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding, Tag, TagMap, TagVariant};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
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

impl CachedData for PeerMessageResponse {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

lazy_static! {
    static ref ENCODING: Encoding = Encoding::Obj(vec![
                Field::new(FieldName::Messages, Encoding::dynamic(Encoding::list(
                    Encoding::Tags(
                        size_of::<u16>(),
                        TagMap::new(&[
                            Tag::new(0x01, TagVariant::Disconnect, Encoding::Unit),
                            Tag::new(0x02, TagVariant::Bootstrap, Encoding::Unit),
                            Tag::new(0x03, TagVariant::Advertise, AdvertiseMessage::encoding()),
                            Tag::new(0x04, TagVariant::SwapRequest, SwapMessage::encoding()),
                            Tag::new(0x05, TagVariant::SwapAck, SwapMessage::encoding()),
                            Tag::new(0x10, TagVariant::GetCurrentBranch, GetCurrentBranchMessage::encoding()),
                            Tag::new(0x11, TagVariant::CurrentBranch, CurrentBranchMessage::encoding()),
                            Tag::new(0x12, TagVariant::Deactivate, DeactivateMessage::encoding()),
                            Tag::new(0x13, TagVariant::GetCurrentHead, GetCurrentHeadMessage::encoding()),
                            Tag::new(0x14, TagVariant::CurrentHead, CurrentHeadMessage::encoding()),
                            Tag::new(0x20, TagVariant::GetBlockHeaders, GetBlockHeadersMessage::encoding()),
                            Tag::new(0x21, TagVariant::BlockHeader, BlockHeaderMessage::encoding()),
                            Tag::new(0x30, TagVariant::GetOperations, GetOperationsMessage::encoding()),
                            Tag::new(0x31, TagVariant::Operation, OperationMessage::encoding()),
                            Tag::new(0x40, TagVariant::GetProtocols, GetProtocolsMessage::encoding()),
                            Tag::new(0x41, TagVariant::Protocol, ProtocolMessage::encoding()),
                            Tag::new(0x50, TagVariant::GetOperationHashesForBlocks, GetOperationHashesForBlocksMessage::encoding()),
                            Tag::new(0x51, TagVariant::OperationHashesForBlocks, OperationHashesForBlocksMessage::encoding()),
                            Tag::new(0x60, TagVariant::GetOperationsForBlocks, GetOperationsForBlocksMessage::encoding()),
                            Tag::new(0x61, TagVariant::OperationsForBlocks, OperationsForBlocksMessage::encoding()),
                        ])
                    )
                )))
            ]);
}

impl HasEncoding for PeerMessageResponse {
    fn encoding() -> Encoding {
        ENCODING.clone()
    }
}

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
