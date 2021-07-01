// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{criterion_group, criterion_main, Criterion};

mod decoding_bench;
use decoding_bench::*;
use tezos_messages::p2p::encoding::{
    ack::AckMessage,
    block_header::{BlockHeaderMessage, GetBlockHeadersMessage},
    connection::ConnectionMessage,
    current_head::{CurrentHeadMessage, GetCurrentHeadMessage},
    operations_for_blocks::{GetOperationsForBlocksMessage, OperationsForBlocksMessage},
    prelude::{CurrentBranchMessage, GetCurrentBranchMessage},
};

fn connection_benchmark(c: &mut Criterion) {
    let data = hex::decode(CONNECTION_DATA).unwrap();
    bench_decode::<ConnectionMessage>(c, "connection", &data);
}

fn ack_benchmark(c: &mut Criterion) {
    let data = hex::decode(ACK_DATA).unwrap();
    bench_decode::<AckMessage>(c, "ack", &data)
}

fn get_current_branch_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-current-branch.msg");
    bench_decode::<GetCurrentBranchMessage>(c, "get-current-branch", &data);
}

fn current_branch_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("current-branch.big.msg");
    bench_decode::<CurrentBranchMessage>(c, "current-branch", &data);
}

fn get_current_head_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-current-head.msg");
    bench_decode::<GetCurrentHeadMessage>(c, "get-current-head", &data);
}

fn current_head_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("current-head.big.msg");
    bench_decode::<CurrentHeadMessage>(c, "current-head", &data);
}

fn get_block_headers_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-block-headers.msg");
    bench_decode::<GetBlockHeadersMessage>(c, "get-block-header", &data);
}

fn block_header_benchmark(c: &mut Criterion) {
    let data = hex::decode(BLOCK_HEADER_DATA).unwrap();
    bench_decode::<BlockHeaderMessage>(c, "block-header", &data);
}

fn get_operations_for_blocks_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("get-operations-for-blocks.msg");
    bench_decode::<GetOperationsForBlocksMessage>(c, "get-operations-for-blocks", &data);
}

fn operations_for_blocks_benchmark(c: &mut Criterion) {
    let data = read_data_unwrap("operations-for-blocks.huge.msg");
    bench_decode::<OperationsForBlocksMessage>(c, "operations-for-blocks", &data);
}

const CONNECTION_DATA: &str = "\
260470387c4cbd115833626a88a9f2b9bbadd70705f5ad059f494655ac823620c6410ec0d9b1716a\
d6d59d0cc551c96992c8031df8c77735d21158dc510aabbca27bd5fff7f085912327012a4d91e587\
e3bc0000000d54455a4f535f4d41494e4e455400000001\
";

const ACK_DATA: &str = "\
01000100000193000000133231332E3233392E3230312E37393A393733320000001237352E313535\
2E36352E3137313A393733320000001334362E3234352E3137392E3136323A393733320000001235\
302E32312E3136372E3231373A393733320000001331382E3138352E3136322E3134343A39373332\
000000133230392E3135392E3134372E37393A39373332000000133130372E3135312E3139302E31\
313A39373332000000123130372E3135312E3139302E363A393733320000001337382E3133372E32\
31352E3133363A393733330000001133352E3138312E33392E31343A39373332000000133130372E\
3135312E3139302E31363A393733320000001231342E3139322E3230392E38343A39373332000000\
133131392E3134372E3231322E31313A39373332000000113231322E34302E34372E31343A393733\
32000000123130372E3135312E3139302E393A393733320000001133342E39352E35372E3134393A\
39373332000000133133382E3230312E37342E3137393A39373332000000133230332E3234332E32\
392E3133373A39373332\
";

const BLOCK_HEADER_DATA: &str = "\
00094F1F048D51777EF01C0106A09F747615CC72271A46EA75E097B48C7200CA2F1EAE6617000000\
005D7F495004C8626895CC82299089F495F7AD8864D1D3B0F364D497F1D175296B5F4A901EC80000\
001100000001000000000800000000012631B27A9F0E1DA2D2CA10202938298CFB1133D5F9A642F8\
1E6697342263B6ECB621F10000000000032DB85C0E00961D14664ECBDF10CBE4DE7DD71096A4E1A1\
77DB0890B13F0AB85999EB0D715E807BCA0438D3CEAA5C58560D60767F28A9E16326657FBE7FC841\
4FDE3C54A504\
";

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = connection_benchmark, ack_benchmark,
    get_current_head_benchmark, current_head_benchmark,
    get_current_branch_benchmark, current_branch_benchmark,
    get_block_headers_benchmark, block_header_benchmark,
    get_operations_for_blocks_benchmark, operations_for_blocks_benchmark,
}

criterion_main!(benches);
