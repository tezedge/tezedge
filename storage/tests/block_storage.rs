// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;

use storage::*;
use storage::tests_common::TmpStorage;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;
use crypto::hash::HashType;

#[test]
fn block_storage_read_write() -> Result<(), Error> {
    let tmp_storage = TmpStorage::create("__block_basictest")?;
    let mut storage = BlockStorage::new(tmp_storage.storage());

    let block_header = make_test_block_header()?;

    storage.put_block_header(&block_header)?;
    let block_header_res = storage.get(&block_header.hash)?.unwrap();
    assert_eq!(block_header_res, block_header);

    Ok(())
}

#[test]
fn block_storage_assign_context() -> Result<(), Error> {
    let tmp_storage = TmpStorage::create("__block_assign_to_context")?;
    let mut storage = BlockStorage::new(tmp_storage.storage());

    let block_header = make_test_block_header()?;
    let context_hash = vec![1; HashType::ContextHash.size()];

    storage.put_block_header(&block_header)?;
    storage.assign_to_context(&block_header.hash, &context_hash)?;
    let block_header_res = storage.get_by_context_hash(&context_hash)?.unwrap();
    assert_eq!(block_header_res, block_header);

    Ok(())
}

fn make_test_block_header() -> Result<BlockHeaderWithHash, Error> {
    let message_bytes = hex::decode("00006d6e0102dd00defaf70c53e180ea148b349a6feb4795610b2abc7b07fe91ce50a90814000000005c1276780432bc1d3a28df9a67b363aa1638f807214bb8987e5f9c0abcbd69531facffd1c80000001100000001000000000800000000000c15ef15a6f54021cb353780e2847fb9c546f1d72c1dc17c3db510f45553ce501ce1de000000000003c762c7df00a856b8bfcaf0676f069f825ca75f37f2bee9fe55ba109cec3d1d041d8c03519626c0c0faa557e778cb09d2e0c729e8556ed6a7a518c84982d1f2682bc6aa753f")?;
    let block_header = BlockHeaderWithHash::new(BlockHeader::from_bytes(message_bytes)?)?;
    Ok(block_header)
}