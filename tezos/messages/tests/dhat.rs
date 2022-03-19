// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fs::File, io::Read, path::PathBuf};

use anyhow::{Context, Error};
use tezos_messages::p2p::{
    binary_message::{BinaryRead, BinaryWrite},
    encoding::{
        ack::AckMessage,
        block_header::{BlockHeaderMessage, GetBlockHeadersMessage},
        connection::ConnectionMessage,
        current_head::{CurrentHeadMessage, GetCurrentHeadMessage},
        operations_for_blocks::{GetOperationsForBlocksMessage, OperationsForBlocksMessage},
        prelude::{CurrentBranchMessage, GetCurrentBranchMessage},
    },
};

fn read_data(file: &str) -> Result<Vec<u8>, Error> {
    let dir = std::env::var("CARGO_MANIFEST_DIR")
        .context("`CARGO_MANIFEST_DIR` is not set".to_string())?;
    let path = PathBuf::from(dir).join("resources").join(file);
    let data = File::open(&path)
        .and_then(|mut file| {
            let mut data = Vec::new();
            file.read_to_end(&mut data)?;
            Ok(data)
        })
        .with_context(|| format!("Cannot read message from {}", path.to_string_lossy()))?;
    Ok(data)
}

fn read_data_unwrap(file: &str) -> Vec<u8> {
    read_data(file).unwrap_or_else(|e| panic!("Unexpected error: {}", e))
}

trait Codec<T> {
    fn decode_encode(file: &str);
}

impl<T> Codec<T> for T
where
    T: BinaryRead + BinaryWrite,
{
    fn decode_encode(file: &str) {
        let data = read_data_unwrap(file);
        let msg = T::from_bytes(data)
            .unwrap_or_else(|e| panic!("Unexpected error while decoding: {}", e));
        msg.as_bytes()
            .unwrap_or_else(|e| panic!("Unexpected error while encoding: {}", e));
    }
}

#[test]
fn all_decode_encode() {
    ConnectionMessage::decode_encode("connection.msg");
    AckMessage::decode_encode("ack.msg");
    GetCurrentBranchMessage::decode_encode("get-current-branch.msg");
    CurrentBranchMessage::decode_encode("current-branch.big.msg");
    GetCurrentHeadMessage::decode_encode("get-current-head.msg");
    CurrentHeadMessage::decode_encode("current-head.big.msg");
    GetBlockHeadersMessage::decode_encode("get-block-headers.msg");
    BlockHeaderMessage::decode_encode("block-header.msg");
    GetOperationsForBlocksMessage::decode_encode("get-operations-for-blocks.msg");
    OperationsForBlocksMessage::decode_encode("operations-for-blocks.huge.msg");
}
