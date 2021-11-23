// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use byteorder::{BigEndian, ByteOrder};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{blake2b::Blake2bError, CryptoError};

use super::blake2b;

const INIT_TO_RESP_SEED: &[u8] = b"Init -> Resp";
const RESP_TO_INIT_SEED: &[u8] = b"Resp -> Init";
pub const NONCE_SIZE: usize = 24;

macro_rules! merge_slices {
    ( $($x:expr),* ) => {{
        let mut res = vec![];
        $(
            res.extend_from_slice($x);
        )*
        res
    }}
}

const NONCE_WORDS: usize = 12;

/// Arbitrary number that can be used once in communication.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct Nonce {
    value: [u16; NONCE_WORDS],
}

impl Nonce {
    /// Create new nonce from raw bytes
    pub fn new(bytes: &[u8]) -> Self {
        assert!(bytes.len() <= NONCE_SIZE);
        let mut _bytes: [u8; NONCE_SIZE] = [0; NONCE_SIZE];
        _bytes.copy_from_slice(bytes);
        let mut value: [u16; NONCE_WORDS] = [0; NONCE_WORDS];
        BigEndian::read_u16_into(&_bytes, &mut value);
        Nonce { value }
    }

    /// Generate new random nonce
    pub fn random() -> Self {
        let mut value: [u16; NONCE_WORDS] = [0; NONCE_WORDS];
        rand::thread_rng().fill(&mut value);
        Nonce { value }
    }

    /// Increment this nonce by one
    pub fn increment(&self) -> Self {
        let mut value: [u16; NONCE_WORDS] = [0; NONCE_WORDS];
        value.copy_from_slice(&self.value);

        let mut pos = NONCE_WORDS - 1;
        loop {
            let result: u32 = value[pos] as u32 + 1u32;
            value[pos] = (result & 0xffffu32) as u16;

            if result < 0x10000u32 || pos == 0 {
                break;
            }

            pos -= 1;
        }

        Nonce { value }
    }

    /// Create bytes representation equal to this nonce with correct nonce size, else return error
    /// TODO: new implementation can't fail so we could get rid of `Result` in return type.
    pub fn get_bytes(&self) -> Result<[u8; NONCE_SIZE], CryptoError> {
        let mut result: [u8; NONCE_SIZE] = [0; NONCE_SIZE];
        BigEndian::write_u16_into(&self.value, &mut result);
        Ok(result)
    }
}

/// Pair of local/remote nonces
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Clone)]
pub struct NoncePair {
    pub local: Nonce,
    pub remote: Nonce,
}

/// Generate NoncePair for incoming/outgoing requests/response message
///
/// # Arguments
/// * `sent_msg` - raw binary message, sent by client
/// * `recv_msg` - raw binary message, received by client
/// * `incoming` - determines, order of messages.
///
/// If incoming is set, `recv_msg` is handled as request and `sent_msg` as response
/// and vice versa if incoming is not set.
pub fn generate_nonces(
    sent_msg: &[u8],
    recv_msg: &[u8],
    incoming: bool,
) -> Result<NoncePair, Blake2bError> {
    let (init_msg, resp_msg) = if incoming {
        (recv_msg, sent_msg)
    } else {
        (sent_msg, recv_msg)
    };

    let nonce_init_to_resp =
        blake2b::digest_256(&merge_slices!(init_msg, resp_msg, INIT_TO_RESP_SEED))?[0..NONCE_SIZE]
            .to_vec();
    let nonce_resp_to_init =
        blake2b::digest_256(&merge_slices!(init_msg, resp_msg, RESP_TO_INIT_SEED))?[0..NONCE_SIZE]
            .to_vec();

    Ok(if incoming {
        NoncePair {
            local: Nonce::new(&nonce_init_to_resp),
            remote: Nonce::new(&nonce_resp_to_init),
        }
    } else {
        NoncePair {
            local: Nonce::new(&nonce_resp_to_init),
            remote: Nonce::new(&nonce_init_to_resp),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_increment_produces_correct_byte_slice() -> Result<(), anyhow::Error> {
        let bytes_0 = hex::decode("0000000000cff52f4be9352787d333e616a67853640d72c5").unwrap();
        let nonce_0 = Nonce::new(&bytes_0);
        let nonce_1 = nonce_0.increment();
        let bytes_1 = nonce_1.get_bytes()?;
        let expected_bytes_1 =
            hex::decode("0000000000cff52f4be9352787d333e616a67853640d72c6").unwrap();
        assert_eq!(expected_bytes_1, bytes_1);

        Ok(())
    }

    #[test]
    fn generate_nonces_produces_correct_results_1() -> Result<(), anyhow::Error> {
        let sent_msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();
        let recv_msg = hex::decode("00874d1ab3845960b32b039fef38ca5c9f8f867df1d522f27a83e07d9dfbe3b296a6c076412d98b369ab015d57247e5380d708b9edfcca0ca2c865346ef9c3d7ed00182cf4f613a6303c9b2a28cda8ff93687bd20000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();

        let NoncePair {
            local: local_nonce,
            remote: remote_nonce,
        } = generate_nonces(&sent_msg, &recv_msg, false)?;
        let expected_local_nonce = "8dde158c55cff52f4be9352787d333e616a67853640d72c5";
        let expected_remote_nonce = "e67481a23cf9b404626a12bd405066e161b32dc53f469153";
        assert_eq!(
            expected_remote_nonce,
            hex::encode(remote_nonce.get_bytes()?)
        );
        assert_eq!(expected_local_nonce, hex::encode(local_nonce.get_bytes()?));

        Ok(())
    }

    #[test]
    fn generate_nonces_produces_correct_results_2() -> Result<(), anyhow::Error> {
        let sent_msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();
        let recv_msg = hex::decode("00874d1ab3845960b32b039fef38ca5c9f8f867df1d522f27a83e07d9dfbe3b296a6c076412d98b369ab015d57247e5380d708b9edfcca0ca2c865346ef9c3d7ed00182cf4f613a6303c9b2a28cda8ff93687bd20000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();

        let NoncePair {
            local: local_nonce,
            remote: remote_nonce,
        } = generate_nonces(&sent_msg, &recv_msg, true)?;
        let expected_local_nonce = "ff0451d94af9f75a46d74a2a9f685cff20222a15829f121d";
        let expected_remote_nonce = "8a09a2c43a61aa6eccee084aa66da9bc94b441b17615be58";
        assert_eq!(
            expected_remote_nonce,
            hex::encode(remote_nonce.get_bytes()?)
        );
        assert_eq!(expected_local_nonce, hex::encode(local_nonce.get_bytes()?));

        Ok(())
    }

    #[test]
    fn too_big_value_wraps() -> Result<(), anyhow::Error> {
        let bytes_0 = hex::decode("ffffffffffffffffffffffffffffffffffffffffffffffff").unwrap();
        let nonce_0 = Nonce::new(&bytes_0);
        let nonce_1 = nonce_0.increment();
        let bytes_1 = nonce_1.get_bytes()?;
        let expected_bytes_1 =
            hex::decode("000000000000000000000000000000000000000000000000").unwrap();
        assert_eq!(expected_bytes_1, bytes_1);
        Ok(())
    }
}
