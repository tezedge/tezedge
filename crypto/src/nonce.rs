// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;

use num_bigint::{BigUint, RandBigInt};

use super::blake2b;

const INIT_TO_RESP_SEED: &[u8] = b"Init -> Resp";
const RESP_TO_INIT_SEED: &[u8] = b"Resp -> Init";
const NONCE_SIZE: usize = 24;

macro_rules! merge_slices {
    ( $($x:expr),* ) => {{
        let mut res = vec![];
        $(
            res.extend_from_slice($x);
        )*
        res
    }}
}

#[derive(Debug, Clone)]
pub struct Nonce {
    value: BigUint
}

impl Nonce {
    pub fn new(bytes: &[u8]) -> Self {
        Nonce {
            value: BigUint::from_bytes_be(bytes)
        }
    }

    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        let value: BigUint = rng.gen_biguint(NONCE_SIZE * 8);
        Nonce { value }
    }

    pub fn increment(&self) -> Self {
        Nonce { value: &self.value + 1u32 }
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut bytes = self.value.to_bytes_be();
        match bytes.len().cmp(&NONCE_SIZE) {
            Ordering::Equal => bytes,
            Ordering::Less => {
                let mut zero_prefixed_bytes = vec![0u8; NONCE_SIZE - bytes.len()];
                zero_prefixed_bytes.append(&mut bytes);
                zero_prefixed_bytes
            },
            Ordering::Greater => panic!("Nonce value overflow"),
        }
    }

    #[allow(dead_code)]
    pub fn get_hash(&self) -> Vec<u8> {
        blake2b::digest_256(&self.value.to_bytes_be())
    }
}

pub struct NoncePair {
    pub local: Nonce,
    pub remote: Nonce,
}

pub fn generate_nonces(sent_msg: &[u8], recv_msg: &[u8], incoming: bool) -> NoncePair {
    let (init_msg, resp_msg) = if incoming { (recv_msg, sent_msg) } else { (sent_msg, recv_msg) };

    let nonce_init_to_resp = blake2b::digest_256(&merge_slices!(init_msg, resp_msg, INIT_TO_RESP_SEED))[0..NONCE_SIZE].to_vec();
    let nonce_resp_to_init = blake2b::digest_256(&merge_slices!(init_msg, resp_msg, RESP_TO_INIT_SEED))[0..NONCE_SIZE].to_vec();

    if incoming {
        NoncePair { local: Nonce::new(&nonce_init_to_resp), remote: Nonce::new(&nonce_resp_to_init) }
    } else {
        NoncePair { local: Nonce::new(&nonce_resp_to_init), remote: Nonce::new(&nonce_init_to_resp) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_increment_produces_correct_byte_slice() {
        let bytes_0 = hex::decode("0000000000cff52f4be9352787d333e616a67853640d72c5").unwrap();
        let nonce_0 = Nonce::new(&bytes_0);
        let nonce_1 = nonce_0.increment();
        let bytes_1 = nonce_1.get_bytes();
        let expected_bytes_1 = hex::decode("0000000000cff52f4be9352787d333e616a67853640d72c6").unwrap();
        assert_eq!(expected_bytes_1, bytes_1)
    }

    #[test]
    fn generate_nonces_produces_correct_results_1() {
        let sent_msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();
        let recv_msg = hex::decode("00874d1ab3845960b32b039fef38ca5c9f8f867df1d522f27a83e07d9dfbe3b296a6c076412d98b369ab015d57247e5380d708b9edfcca0ca2c865346ef9c3d7ed00182cf4f613a6303c9b2a28cda8ff93687bd20000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();

        let NoncePair { local: local_nonce, remote: remote_nonce } = generate_nonces(&sent_msg, &recv_msg, false);
        let expected_local_nonce = "8dde158c55cff52f4be9352787d333e616a67853640d72c5";
        let expected_remote_nonce = "e67481a23cf9b404626a12bd405066e161b32dc53f469153";
        assert_eq!(expected_remote_nonce, hex::encode(remote_nonce.get_bytes()));
        assert_eq!(expected_local_nonce, hex::encode(local_nonce.get_bytes()));
    }

    #[test]
    fn generate_nonces_produces_correct_results_2() {
        let sent_msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();
        let recv_msg = hex::decode("00874d1ab3845960b32b039fef38ca5c9f8f867df1d522f27a83e07d9dfbe3b296a6c076412d98b369ab015d57247e5380d708b9edfcca0ca2c865346ef9c3d7ed00182cf4f613a6303c9b2a28cda8ff93687bd20000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000").unwrap();

        let NoncePair { local: local_nonce, remote: remote_nonce } = generate_nonces(&sent_msg, &recv_msg, true);
        let expected_local_nonce = "ff0451d94af9f75a46d74a2a9f685cff20222a15829f121d";
        let expected_remote_nonce = "8a09a2c43a61aa6eccee084aa66da9bc94b441b17615be58";
        assert_eq!(expected_remote_nonce, hex::encode(remote_nonce.get_bytes()));
        assert_eq!(expected_local_nonce, hex::encode(local_nonce.get_bytes()));
    }

    #[test]
    #[should_panic]
    fn too_big_value_produces_panic() {
        let nonce = Nonce::new(&[0x1F; NONCE_SIZE + 1]);
        let _ = nonce.get_bytes();
    }
}