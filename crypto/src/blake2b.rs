// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use sodiumoxide::crypto::generichash::State;

pub fn digest_256(data: &[u8]) -> Vec<u8> {
    digest(data, 32)
}

pub fn digest_128(data: &[u8]) -> Vec<u8> {
    digest(data, 16)
}

fn digest(data: &[u8], out_len: usize) -> Vec<u8> {
    let mut hasher = State::new(out_len, None).unwrap();
    hasher.update(data).expect("Failed to update hasher state");

    let hash = hasher.finalize().unwrap();
    let mut result = Vec::new();
    result.extend_from_slice(hash.as_ref());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2b_256() {
        let hash = digest_256(b"hello world");
        let expected = hex::decode("256c83b297114d201b30179f3f0ef0cace9783622da5974326b436178aeef610").unwrap();
        assert_eq!(expected, hash)
    }
}