use rpc::{make_json_response, ServiceResult};
use serde_json::Value;
use cached::proc_macro::cached;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

#[cached(result = true)]
pub fn cached_data_response_wrapped() -> ServiceResult {
    make_json_response(&data())
}

pub fn cached_data() -> ServiceResult {
    make_json_response(&data())
}
#[cached]
pub fn data() -> Value {
    serde_json::json!({
        "name" : "hello world"
    })
}


pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("cached_data_response_wrapped", |b| b.iter(|| cached_data_response_wrapped()));
    c.bench_function("cached_data", |b| b.iter(|| cached_data()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);