// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use sodiumoxide::crypto::generichash::State;
use failure::Fail;

#[derive(Debug, Copy, Clone, Fail)]
#[fail(display = "Output digest length must be between 16 and 64 bytes.")]
pub struct Blake2bLengthError;

/// Generate digest of length 256 bits (32bytes) from arbitrary binary data
pub fn digest_256(data: &[u8]) -> Vec<u8> {
    digest(data, 32)
        .expect("Blake2b unexpectedly failed on correct digest length")
}

// Generate digest of length 160 bits (20bytes) from arbitrary binary data
pub fn digest_160(data: &[u8]) -> Vec<u8> {
    digest(data, 20)
        .expect("Blake2b unexpectedly failed on correct digest length")
}

/// Generate digest of length 256 bits (32bytes) from arbitrary binary data
pub fn digest_128(data: &[u8]) -> Vec<u8> {
    digest(data, 16)
        .expect("Blake2b unexpectedly failed on correct digest length")
}

/// Arbitrary Blake2b digest generation from generic data.
// Should be noted, that base Blake2b supports arbitrary digest length from 16 to 64 bytes
fn digest(data: &[u8], out_len: usize) -> Result<Vec<u8>, Blake2bLengthError> {
    let mut hasher = State::new(out_len, None).map_err(|_| Blake2bLengthError)?;
    hasher.update(data).expect("Failed to update hasher state");

    let hash = hasher.finalize().unwrap();
    let mut result = Vec::with_capacity(out_len);
    result.extend_from_slice(hash.as_ref());
    Ok(result)
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

    #[test]
    fn blake2b_128() {
        let hash = digest_128(b"hello world");
        let expected = hex::decode("e9a804b2e527fd3601d2ffc0bb023cd6").unwrap();
        assert_eq!(expected, hash);
    }

    #[test]
    fn blake2b_less_than_128() {
        // This should fail, as blake2b does not support hashes shorter than 16 bytes.
        assert!(digest(b"hello world", 15).is_err())
    }

    #[test]
    fn blake2b_more_than_512() {
        // This should fail, as blake2b does not support hashes longer than 64 bytes.
        assert!(digest(b"hello world", 65).is_err())
    }
}