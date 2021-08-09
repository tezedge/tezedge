// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

const COMPRESSION_LEVEL: i32 = 2;

pub fn zstd_compress<B: AsRef<[u8]>>(input: B, output: &mut Vec<u8>) -> std::io::Result<()> {
    zstd::stream::copy_encode(input.as_ref(), output, COMPRESSION_LEVEL)
}

pub fn zstd_decompress<B: AsRef<[u8]>>(input: B, output: &mut Vec<u8>) -> std::io::Result<()> {
    zstd::stream::copy_decode(input.as_ref(), output)
}
