#![no_main]
use libfuzzer_sys::fuzz_target;
use tezos_encoding::binary_reader::BinaryReader;

fn binary_reader_decode_data(data: &[u8]) {
    let binary_reader = BinaryReader::new();
    if let Err(e) = binary_reader.read(data.to_vec()) {
        panic!("BinaryReader::read failed with error: {:?}", e);
    }
}

fuzz_target!(|data: &[u8]| {
    binary_reader_decode_data(data)
});
