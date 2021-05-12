#![allow(dead_code)]

use std::{fs::File, io::Read, path::PathBuf};

use criterion::{black_box, Criterion};
use failure::Error;
use tezos_messages::p2p::binary_message::{BinaryMessageNom, BinaryMessageRaw, BinaryMessageSerde};

pub fn read_data(file: &str) -> Result<Vec<u8>, Error> {
    let dir = std::env::var("CARGO_MANIFEST_DIR")?;
    let path = PathBuf::from(dir).join("resources").join(file);
    let data = File::open(path).and_then(|mut file| {
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;
        Ok(data)
    })?;
    Ok(data)
}

pub fn read_data_unwrap(file: &str) -> Vec<u8> {
    read_data(file).unwrap_or_else(|e| panic!("Unexpected error: {}", e))
}

fn bench_decode_serde<T: BinaryMessageSerde>(c: &mut Criterion, message: &str, data: &Vec<u8>) {
    c.bench_function(&format!("{}-decode-serde", message), |b| {
        b.iter(|| {
            let _ = black_box(<T as BinaryMessageSerde>::from_bytes(&data))
                .expect("Failed to decode with serde");
        });
    });
}

fn bench_decode_nom<T: BinaryMessageNom>(c: &mut Criterion, message: &str, data: &Vec<u8>) {
    c.bench_function(&format!("{}-decode-nom", message), |b| {
        b.iter(|| {
            let _ = black_box(<T as BinaryMessageNom>::from_bytes(&data))
                .expect("Failed to decode with nom");
        });
    });
}

fn bench_decode_raw<T: BinaryMessageRaw>(c: &mut Criterion, message: &str, data: &Vec<u8>) {
    c.bench_function(&format!("{}-decode-raw", message), |b| {
        b.iter(|| {
            let _ = black_box(<T as BinaryMessageRaw>::from_bytes(&data))
                .expect("Failed to decode with raw");
        });
    });
}

pub fn bench_decode_serde_nom<T: BinaryMessageSerde + BinaryMessageNom>(
    c: &mut Criterion,
    message: &str,
    data: Vec<u8>,
) {
    bench_decode_serde::<T>(c, message, &data);
    bench_decode_nom::<T>(c, message, &data);
}

pub fn bench_decode_serde_nom_raw<T: BinaryMessageSerde + BinaryMessageNom + BinaryMessageRaw>(
    c: &mut Criterion,
    message: &str,
    data: Vec<u8>,
) {
    bench_decode_serde::<T>(c, message, &data);
    bench_decode_nom::<T>(c, message, &data);
    bench_decode_raw::<T>(c, message, &data);
}
