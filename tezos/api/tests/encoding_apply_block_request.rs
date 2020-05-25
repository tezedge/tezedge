// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Error;

use crypto::hash::HashType;
use tezos_api::ffi::{APPLY_BLOCK_REQUEST_ENCODING, ApplyBlockRequest, ApplyBlockRequestBuilder};
use tezos_encoding::{binary_writer, de};
use tezos_encoding::binary_reader::BinaryReader;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

#[test]
fn can_serde() -> Result<(), Error> {

    // request
    let orig_request: ApplyBlockRequest = ApplyBlockRequestBuilder::default()
        .chain_id(HashType::ChainId.string_to_bytes("NetXgtSLGNJvNye")?)
        .block_header(BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap()).unwrap())
        .pred_header(BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap()).unwrap())
        .max_operations_ttl(2)
        .operations(
            ApplyBlockRequest::convert_operations(
                &test_data::block_operations_from_hex(
                    test_data::BLOCK_HEADER_HASH_LEVEL_3,
                    test_data::block_header_level3_operations(),
                )
            )
        )
        .build().unwrap();

    // write to bytes
    let request = binary_writer::write(&orig_request, &APPLY_BLOCK_REQUEST_ENCODING).expect("Failed to create request");

    // test
    assert_eq!(
        &hex::encode(&request),
        "8eceda2f000000ce0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605000000ce0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf0500000002000000790000006900000065a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a000000000000000000000000"
    );

    // reader
    let decoded_request = BinaryReader::new().read(request, &APPLY_BLOCK_REQUEST_ENCODING)?;
    let decoded_request: ApplyBlockRequest = de::from_value(&decoded_request)?;

    assert_eq!(orig_request.chain_id, decoded_request.chain_id);
    assert_eq!(orig_request.block_header, decoded_request.block_header);
    assert_eq!(orig_request.pred_header, decoded_request.pred_header);
    assert_eq!(orig_request.max_operations_ttl, decoded_request.max_operations_ttl);
    assert_eq!(orig_request.operations, decoded_request.operations);

    Ok(())
}

mod test_data {
    use tezos_messages::p2p::binary_message::BinaryMessage;
    use tezos_messages::p2p::encoding::prelude::*;

    // BLwKksYwrxt39exDei7yi47h7aMcVY2kZMZhTwEEoSUwToQUiDV
    pub const BLOCK_HEADER_LEVEL_2: &str = "0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05";

    // BLTQ5B4T4Tyzqfm3Yfwi26WmdQScr6UXVSE9du6N71LYjgSwbtc
    pub const BLOCK_HEADER_HASH_LEVEL_3: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
    pub const BLOCK_HEADER_LEVEL_3: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";

    pub fn block_header_level3_operations() -> Vec<Vec<String>> {
        vec![
            vec!["a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a".to_string()],
            vec![],
            vec![],
            vec![]
        ]
    }

    pub fn block_operations_from_hex(block_hash: &str, hex_operations: Vec<Vec<String>>) -> Vec<Option<OperationsForBlocksMessage>> {
        hex_operations
            .into_iter()
            .map(|bo| {
                let ops = bo
                    .into_iter()
                    .map(|op| Operation::from_bytes(hex::decode(op).unwrap()).unwrap())
                    .collect();
                Some(OperationsForBlocksMessage::new(OperationsForBlock::new(hex::decode(block_hash).unwrap(), 4), Path::Op, ops))
            })
            .collect()
    }
}