// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn ack_benchmark(c: &mut Criterion) {
    use tezos_messages::p2p::encoding::ack::AckMessage;
    let data = hex::decode("01000100000193000000133231332E3233392E3230312E37393A393733320000001237352E3135352E36352E3137313A393733320000001334362E3234352E3137392E3136323A393733320000001235302E32312E3136372E3231373A393733320000001331382E3138352E3136322E3134343A39373332000000133230392E3135392E3134372E37393A39373332000000133130372E3135312E3139302E31313A39373332000000123130372E3135312E3139302E363A393733320000001337382E3133372E3231352E3133363A393733330000001133352E3138312E33392E31343A39373332000000133130372E3135312E3139302E31363A393733320000001231342E3139322E3230392E38343A39373332000000133131392E3134372E3231322E31313A39373332000000113231322E34302E34372E31343A39373332000000123130372E3135312E3139302E393A393733320000001133342E39352E35372E3134393A39373332000000133133382E3230312E37342E3137393A39373332000000133230332E3234332E32392E3133373A39373332").unwrap();
    let serde_res = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageSerde>::from_bytes(&data).unwrap();
    let nom_res = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageSerde>::from_bytes(&data).unwrap();
    let raw_res = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageSerde>::from_bytes(&data).unwrap();
    assert_eq!(serde_res, nom_res);
    assert_eq!(serde_res, raw_res);
    c.bench_function("ack_decode_serde", |b| {
        b.iter(|| {
            let _ = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageSerde>::from_bytes(&data).unwrap();
        });
    });
    c.bench_function("ack_decode_nom", |b| {
        b.iter(|| {
            let _ = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageNom>::from_bytes(&data).unwrap();
        });
    });
    c.bench_function("ack_decode_raw", |b| {
        b.iter(|| {
            let _ = <AckMessage as tezos_messages::p2p::binary_message::BinaryMessageRaw>::from_bytes(&data).unwrap();
        });
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = ack_benchmark
}

criterion_main!(benches);
