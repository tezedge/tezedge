// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;
use crypto::hash::HashType;
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::encoding::Encoding;
use tezos_encoding::types::Value;

#[test]
fn can_decode_hash() -> Result<(), Error> {
    let mut buf =  [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31].as_ref();
    let list:Vec<Value> = buf.into_iter().map(|x|Value::Uint8(*x)).collect();
    let br = BinaryReader::new();
    let read_hash = br.read(&mut buf, &Encoding::Hash(HashType::PublicKeyEd25519)).unwrap();
    assert_eq!(Value::List(list), read_hash);
    Ok(())
}