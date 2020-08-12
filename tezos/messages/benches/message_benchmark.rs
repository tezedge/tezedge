use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tezos_messages::p2p::{binary_message::BinaryMessage, encoding::operation::Operation, binary_message::cache::CachedData};
use tezos_encoding::{binary_reader::BinaryReader, encoding::HasEncoding};
use tezos_encoding::de::from_value as deserialize_from_value;

// deserialize_benchmark mimics execution of main operations in BinaryMessage::from_bytes
pub fn deserialize_benchmark(c: &mut Criterion) {
    let message_bytes = hex::decode("0090304939374e4f0f260928d4879fd5f359b4ff146f3fd37142436fb8ce1ab57af68648964efff6ca56a82b61185aec6538fa000125a2a1468416d65247660efcba15111467b9feab07dfc3dafac2d2a8c4c6dbca0b97b7239bcc4bd7ab2229b9c506022870539f6505ff56af81e5d344baa82465bae2a023afa5de27a6600e4dc85b050471ef8c3d887bb7a65700caaa98").unwrap();
    c.bench_function("operation_from_bytes", |b| b.iter(|| { Operation::from_bytes(black_box(message_bytes.clone()))}));
    c.bench_function("from_bytes_reader", |b| b.iter(|| { BinaryReader::new().read(black_box(message_bytes.clone()), &Operation::encoding()).unwrap() }));
    let value = BinaryReader::new().read(black_box(message_bytes.clone()), &Operation::encoding()).unwrap();
    c.bench_function("from_bytes_deserialize", |b| b.iter(|| deserialize_from_value::<Operation>(black_box(&value)).unwrap()));
    let mut msg: Operation = deserialize_from_value(&value).unwrap();
    if let Some(cache_writer) = msg.cache_writer() {
        c.bench_function("from_bytes_write_cache", |b| b.iter(|| cache_writer.put(black_box(&message_bytes)) ));
    }
}

criterion_group!(benches, deserialize_benchmark);
criterion_main!(benches);
