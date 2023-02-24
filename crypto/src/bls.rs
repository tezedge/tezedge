// SPDX-FileCopyrightText: 2022-2023 TriliTech <contact@trili.tech>
//
// SPDX-License-Identifier: MIT

//! BLS support (min_key).

use std::fmt::Debug;

use crate::hash::BlsSignature;
use crate::hash::ContractTz4Hash;
use crate::hash::PublicKeyBls;
use crate::PublicKeyWithHash;
use blst::min_pk;
use blst::min_pk::{AggregateSignature, SecretKey};
use blst::BLST_ERROR;
use thiserror::Error;

/// Wrapper for all errors coming from `blst`
///
/// Wrapping of the C style return code used by blst. Functions in this
/// module should avoid ever returning something like `BlsError(BLST_SUCCESS)`.
#[derive(Debug, PartialEq, Error, Clone)]
pub struct BlsError(BLST_ERROR);

impl std::fmt::Display for BlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "BlsError {:?}", self.0)
    }
}

/// Transform `blst` errors to Rust `Result`.
///
/// Go from `BLST_ERROR` to normal Rust `Result`, where a success is
/// interpreted as true, ie, it is true that signature is valid and
/// false means signature is invalid. Errors can be caused by invalid
/// public keys, et.c.
fn bool_result_from(blst_error: BLST_ERROR) -> Result<bool, BlsError> {
    match blst_error {
        BLST_ERROR::BLST_SUCCESS => Ok(true),
        BLST_ERROR::BLST_VERIFY_FAIL => Ok(false),
        _ => Err(BlsError(blst_error)),
    }
}

impl TryFrom<&PublicKeyBls> for min_pk::PublicKey {
    type Error = BlsError;

    fn try_from(source: &PublicKeyBls) -> Result<Self, Self::Error> {
        min_pk::PublicKey::from_bytes(&source.0).map_err(BlsError)
    }
}

/// A message and a public key for this message.
///
/// This follows the `aggregate_verify` calling convention used for TORU.
/// Not currently being used, but the mode is available in the tezos protocol.
pub type Message<'a> = (&'a [u8], &'a PublicKeyBls);

/// Basic `dst` parameter for `blst`
///
/// Same constant as used for `Basic` in `bls12-381` ocaml package used in tezos.
#[allow(dead_code)]
const BASIC_CIPHER_SUITE: &str = "BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_";

/// Aug `dst` parameter for `blst`
///
/// Same constant as used for `Basic` in `bls12-381` ocaml package used in tezos.
/// This is the mode used for TORU.
const AUG_CIPHER_SUITE: &str = "BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";

/// Pop signatures parameter value for `blst`
///
/// Same constant as used for verify `Pop` in `bls12-381` ocaml package used in tezos.
/// Not currently being used, but the mode is available in the tezos protocol.
#[allow(dead_code)]
const POP_CIPHER_SUITE: &str = "BLS_POP_BLS12381G2_XMD:SHA-256_SSWU_RO_POP_";

impl BlsSignature {
    /// Verify several messages with public keys and _one_ signature.
    ///
    /// Verify a signature for one or more messages. The signature must have been
    /// constructed by *only* the given messages.
    ///
    /// When verifying, we also check the public keys and group check. Like Tezos_crypto,
    /// this verification function uses the Aug suite.
    pub fn aggregate_verify<'a>(
        &self,
        messages: &mut impl Iterator<Item = Message<'a>>,
    ) -> Result<bool, BlsError> {
        let signature: min_pk::Signature = self.try_into()?;

        let messages_with_pk = messages
            .map(|(message, public_key)| {
                // For saftey, we ensure that each message is unique by prepending the
                // public key.
                let message_with_pk = prepend_public_key(message, public_key);

                public_key
                    .try_into()
                    .map(move |public_key| (message_with_pk, public_key))
            })
            .collect::<Result<Vec<(Vec<u8>, min_pk::PublicKey)>, _>>()?;

        let messages = messages_with_pk
            .iter()
            .map(|(message, _)| message.as_slice())
            .collect::<Vec<_>>();

        let public_keys = messages_with_pk
            .iter()
            .map(|(_, pk)| pk)
            .collect::<Vec<_>>();

        // Tezos_crypto uses the Aug suite
        let dst = AUG_CIPHER_SUITE.as_bytes();

        bool_result_from(signature.aggregate_verify(true, &messages, dst, &public_keys, true))
    }

    /// Aggregate individual signatures into a single signature.
    pub fn aggregate_sigs(sigs: &[&Self]) -> Result<Self, BlsError> {
        let sigs = sigs
            .iter()
            .map(|s| min_pk::Signature::try_from(*s))
            .collect::<Result<Vec<_>, _>>()?;

        let sigs = sigs.iter().collect::<Vec<_>>();

        let aggregate = AggregateSignature::aggregate(sigs.as_slice(), true).map_err(BlsError)?;

        aggregate.validate().map_err(BlsError)?;

        Ok(Self(aggregate.to_signature().compress().to_vec()))
    }
}

impl TryFrom<&BlsSignature> for min_pk::Signature {
    type Error = BlsError;

    fn try_from(value: &BlsSignature) -> Result<Self, Self::Error> {
        min_pk::Signature::from_bytes(&value.0).map_err(BlsError)
    }
}

/// Wrapper of a [SecretKey], alongside its public key.
#[derive(Clone)]
pub struct BlsKey {
    sk: SecretKey,
    pk: PublicKeyBls,
    pk_hash: ContractTz4Hash,
}

impl BlsKey {
    /// Public key of the contained secret key.
    pub fn public_key(&self) -> &PublicKeyBls {
        &self.pk
    }

    /// Hash of the public key - a.k.a a *Layer 2* address.
    pub fn public_key_hash(&self) -> &ContractTz4Hash {
        &self.pk_hash
    }

    /// Sign a message, using the *aug* cipher suite.
    pub fn sign(&self, msg: &[u8]) -> BlsSignature {
        let message_with_pk = prepend_public_key(msg, &self.pk);

        BlsSignature(
            self.sk
                .sign(&message_with_pk, AUG_CIPHER_SUITE.as_bytes(), &[])
                .to_bytes()
                .to_vec(),
        )
    }

    /// Deterministically generate a `BlsKey` from initial key material.
    pub fn from_ikm(ikm: [u8; 32]) -> Self {
        let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
        let pk = PublicKeyBls(sk.sk_to_pk().to_bytes().to_vec());
        let pk_hash = pk.pk_hash().unwrap();

        BlsKey { sk, pk, pk_hash }
    }
}

impl PartialEq for BlsKey {
    /// Equality by comparison of [ContractTz4Hash].
    fn eq(&self, other: &Self) -> bool {
        self.pk_hash == other.pk_hash
    }
}

impl Debug for BlsKey {
    /// Debug [BlsKey] must not expose the [SecretKey].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            f,
            "BlsKey(\"{}\")",
            self.public_key_hash().to_base58_check()
        )
    }
}

// We prepend each message with the public key used to sign it.
fn prepend_public_key(msg: &[u8], pk: &PublicKeyBls) -> Vec<u8> {
    let mut message_with_pk =
        Vec::with_capacity(msg.len() + crate::hash::HashType::PublicKeyP256.size());
    message_with_pk.extend_from_slice(&pk.0);
    message_with_pk.extend_from_slice(msg);
    message_with_pk
}

#[cfg(any(test, feature = "rand"))]
mod bls_gen {
    //! Generation of Bls keys used for testing.
    //!
    //! Use
    //! - `key in BlsKey::arb()` during Property Based Tests.
    //! - `let key: BlsKey = Faker.fake()` in unit tests, when hardcoding a key is
    //!   undesirable.
    use super::*;
    use proptest::prelude::*;
    use rand::Rng;

    impl BlsKey {
        /// Generate arbitrary bls keys for testing
        pub fn arb() -> BoxedStrategy<BlsKey> {
            any::<[u8; 32]>().prop_map(Self::from_ikm).boxed()
        }

        /// Generate random bls key
        pub fn generate() -> Self {
            let ikm = rand::thread_rng().gen::<[u8; 32]>();
            Self::from_ikm(ikm)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        base58::FromBase58Check,
        hash::{ContractTz4Hash, HashTrait},
        PublicKeyWithHash,
    };
    use proptest::prelude::*;

    #[test]
    fn decoding_invalid_signature_gives_error() {
        use super::BlsError;
        use blst::BLST_ERROR;

        let bytes: [u8; 96] = [
            212, 164, 10, 104, 205, 205, 23, 255, 218, 184, 156, 159, 150, 133, 185, 31, 221, 34,
            11, 39, 189, 17, 16, 28, 109, 72, 109, 48, 239, 105, 121, 121, 100, 149, 1, 168, 106,
            118, 145, 148, 182, 122, 206, 83, 8, 214, 146, 238, 181, 41, 182, 23, 221, 66, 47, 99,
            179, 9, 195, 96, 141, 204, 99, 53, 222, 157, 64, 102, 177, 118, 26, 240, 235, 189, 109,
            214, 229, 77, 77, 24, 53, 136, 220, 124, 102, 108, 5, 241, 185, 98, 145, 206, 121, 169,
            11, 255,
        ];

        let sig = BlsSignature(bytes.to_vec());
        let sig = min_pk::Signature::try_from(&sig);

        assert_eq!(sig, Err(BlsError(BLST_ERROR::BLST_BAD_ENCODING)));
    }

    #[test]
    fn decoding_valid_signature_is_ok() {
        let bytes: [u8; 96] = [
            149, 240, 234, 160, 166, 30, 18, 15, 229, 113, 68, 192, 204, 118, 169, 78, 252, 237,
            251, 111, 240, 127, 236, 68, 231, 114, 243, 76, 61, 156, 148, 34, 203, 153, 6, 255,
            159, 108, 21, 71, 163, 120, 87, 133, 239, 135, 225, 127, 14, 126, 215, 20, 107, 206,
            222, 198, 187, 11, 173, 56, 167, 119, 182, 55, 57, 102, 180, 194, 18, 91, 49, 59, 130,
            39, 33, 103, 243, 211, 156, 164, 53, 160, 255, 198, 58, 2, 124, 121, 201, 44, 139, 167,
            48, 52, 211, 178,
        ];

        let sig = BlsSignature(bytes.to_vec());
        let sig = min_pk::Signature::try_from(&sig);

        assert!(sig.is_ok());
    }

    #[test]
    fn return_error_on_invalid_public_key_encoding() {
        use super::{min_pk, BlsError, PublicKeyBls};
        use blst::BLST_ERROR;

        let bytes: [u8; 48] = [
            118, 187, 155, 125, 42, 190, 144, 143, 145, 250, 125, 184, 90, 9, 210, 24, 202, 72, 22,
            137, 121, 174, 233, 107, 175, 63, 167, 107, 192, 38, 60, 102, 74, 213, 169, 88, 51,
            181, 190, 79, 226, 209, 166, 137, 54, 88, 14, 28,
        ];

        let pk = PublicKeyBls(bytes.to_vec());
        let pk = min_pk::PublicKey::try_from(&pk);

        assert_eq!(pk, Err(BlsError(BLST_ERROR::BLST_BAD_ENCODING)));
    }

    #[test]
    fn decode_valid_public_key_is_ok() {
        let bytes: [u8; 48] = [
            145, 150, 3, 232, 142, 40, 236, 191, 116, 53, 15, 47, 240, 143, 182, 94, 110, 121, 160,
            108, 247, 42, 34, 231, 133, 156, 81, 111, 62, 109, 59, 223, 198, 220, 89, 7, 173, 251,
            241, 82, 161, 86, 161, 40, 141, 57, 145, 123,
        ];

        let pk = PublicKeyBls(bytes.to_vec());
        let pk = min_pk::PublicKey::try_from(&pk);

        assert!(pk.is_ok());
    }

    #[test]
    fn can_verify_signature_is_true() {
        use super::AUG_CIPHER_SUITE;
        use blst::min_pk::SecretKey;

        let ikm: [u8; 32] = [
            206, 83, 215, 142, 19, 242, 183, 160, 92, 186, 87, 192, 89, 109, 82, 0, 17, 60, 248,
            194, 149, 144, 24, 238, 202, 18, 75, 107, 139, 241, 104, 198,
        ];

        let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
        let pk = PublicKeyBls(sk.sk_to_pk().to_bytes().to_vec());

        let dst = AUG_CIPHER_SUITE.as_bytes();

        let msg = b"blst is such a blast";
        let mut signed_bytes = Vec::new();
        signed_bytes.extend_from_slice(&pk.0);
        signed_bytes.extend_from_slice(msg);

        let sig = BlsSignature(sk.sign(&signed_bytes, dst, &[]).to_bytes().to_vec());

        let msg_keys = [(&msg[..], &pk)];
        let res = sig.aggregate_verify(&mut msg_keys.into_iter());

        assert_eq!(res, Ok(true));
    }

    // Values taken from tezt test, that was failing due to public key not being
    // prepended to msg.
    #[test]
    fn bls_sign_with_public_key() {
        use super::*;

        let ikm = [0; 32];
        let key = BlsKey::from_ikm(ikm);

        assert_eq!(
            "tz4TpX5Qb3w7xnnnwSpjFs7Kq35GC4qr3uMg",
            &key.pk_hash.to_base58_check(),
            "expected addresses to match"
        );

        let expected_pk_bytes = &[
            166, 149, 173, 50, 93, 252, 126, 17, 145, 251, 201, 241, 134, 245, 142, 255, 66, 166,
            52, 2, 151, 49, 177, 131, 128, 255, 137, 191, 66, 196, 100, 164, 44, 184, 202, 85, 178,
            0, 240, 81, 245, 127, 30, 24, 147, 198, 135, 89,
        ];

        let actual_pk_bytes = key.pk.0.as_slice();
        assert_eq!(expected_pk_bytes, actual_pk_bytes, "expected pk to match");

        let msg_bytes = &[
            0, 0, 0, 141, 0, 166, 149, 173, 50, 93, 252, 126, 17, 145, 251, 201, 241, 134, 245,
            142, 255, 66, 166, 52, 2, 151, 49, 177, 131, 128, 255, 137, 191, 66, 196, 100, 164, 44,
            184, 202, 85, 178, 0, 240, 81, 245, 127, 30, 24, 147, 198, 135, 89, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 80, 0, 241, 81, 248, 184, 241, 123, 33, 174, 242, 58, 123, 212, 112,
            246, 137, 34, 107, 182, 199, 80, 7, 7, 10, 0, 0, 0, 22, 1, 98, 180, 106, 212, 106, 51,
            26, 188, 121, 202, 100, 202, 43, 197, 198, 234, 211, 47, 205, 180, 0, 7, 7, 1, 0, 0, 0,
            1, 82, 0, 143, 6, 0, 0, 0, 15, 114, 101, 99, 101, 105, 118, 101, 95, 116, 105, 99, 107,
            101, 116, 115,
        ];

        let sig = key.sign(msg_bytes);

        let expected_sig = &[
            170, 60, 254, 35, 122, 181, 133, 165, 130, 161, 54, 157, 133, 39, 163, 49, 146, 56, 61,
            74, 35, 132, 225, 148, 164, 71, 198, 188, 66, 209, 192, 174, 163, 157, 81, 131, 94,
            154, 42, 8, 96, 122, 180, 162, 81, 68, 181, 240, 11, 73, 110, 14, 205, 72, 139, 77,
            134, 106, 133, 253, 138, 75, 56, 85, 188, 232, 58, 186, 100, 124, 234, 69, 200, 163,
            162, 173, 252, 147, 139, 60, 247, 86, 162, 6, 65, 142, 190, 178, 244, 190, 137, 30, 37,
            32, 238, 240,
        ];

        assert_eq!(
            expected_sig,
            sig.0.as_slice(),
            "expected signatures to match"
        );
    }

    #[test]
    fn can_verify_signature_is_false() {
        use blst::min_pk::SecretKey;

        let ikm: [u8; 32] = [
            139, 238, 61, 128, 196, 109, 58, 44, 13, 240, 207, 148, 246, 216, 242, 161, 132, 197,
            169, 201, 120, 146, 252, 112, 92, 255, 57, 102, 202, 178, 210, 113,
        ];

        let signature_bytes: [u8; 96] = [
            149, 240, 234, 160, 166, 30, 18, 15, 229, 113, 68, 192, 204, 118, 169, 78, 252, 237,
            251, 111, 240, 127, 236, 68, 231, 114, 243, 76, 61, 156, 148, 34, 203, 153, 6, 255,
            159, 108, 21, 71, 163, 120, 87, 133, 239, 135, 225, 127, 14, 126, 215, 20, 107, 206,
            222, 198, 187, 11, 173, 56, 167, 119, 182, 55, 57, 102, 180, 194, 18, 91, 49, 59, 130,
            39, 33, 103, 243, 211, 156, 164, 53, 160, 255, 198, 58, 2, 124, 121, 201, 44, 139, 167,
            48, 52, 211, 178,
        ];

        let sk = SecretKey::key_gen(&ikm, &[]).unwrap();
        let pk = PublicKeyBls(sk.sk_to_pk().to_bytes().to_vec());

        let msg = b"blst is such a blast";
        let sig = BlsSignature(signature_bytes.to_vec());

        let msg_keys = [(&msg[..], &pk)];
        let res = sig.aggregate_verify(&mut msg_keys.into_iter());

        assert_eq!(res, Ok(false));
    }

    // Test to ensure that we use the correct hashing scheme to convert between
    // bls::PublicKey and ContractTz4Hash.
    //
    // Test cases generated using protocol unit tests for tx_rollup.
    #[test]
    fn ensure_public_key_hashing_correct() {
        let test_cases = [
            (
                "BLpk1mJXuRWVxRJRkUES7E16u4KeKMGibFtR995FxSFzeyMm8ckngdo4Cx4P3KWeRQ1NeY3iEWwq",
                "tz4NPp9xHJMgoRQwE2iL66NnfeBcnEoskeuj",
            ),
            (
                "BLpk1wEURk8sJBP2QvjNnjFiFbqJJfRYSKAjHJXbAD9rU1h4wpn8Q1wUAsVXGK3bRrLvhT8f5o3z",
                "tz492MCfwp9V961DhNGmKzD642uhU8j6H5nB",
            ),
            (
                "BLpk1pJuYBQjSb1JhnfNTkAr2AJ4BhcMdTfyQBK97EHLMukaCWkqpGNxP6fkpZcmLbux7UPNqJhP",
                "tz4Vc8F1uDc5vxppuWhAvu2HpjsFYS5x5qat",
            ),
            (
                "BLpk1rk1RFdkPgdv5V2PDmxnL3gDeUcQCmH241rGb8YeRqtpAHPnWeUGkvBmzurXCKJJZEsHrjL8",
                "tz4FJr811sHV649iKrFFgrFM7mvSYBQSKHsP",
            ),
        ];

        let run_test = |(pk_b58, tz4_b58): (&str, &str)| {
            let pk_bytes = pk_b58.from_base58check().expect("Valid pk b58");
            let pk_bytes: [u8; 48] = pk_bytes[4..] // remove prefix of `BLpk`
                .try_into()
                .expect("pk_bytes should be 48 bytes long");
            let pk = PublicKeyBls(pk_bytes.to_vec());

            let tz4 = pk.pk_hash().expect("PublicKey hashable");

            let expected_tz4 = ContractTz4Hash::from_b58check(tz4_b58).expect("Valid tz4 b58");

            assert_eq!(expected_tz4, tz4);
        };

        test_cases.into_iter().for_each(run_test);
    }

    proptest! {
      #[test]
      fn verify_signature_of_single_pk(key in BlsKey::arb(), msg in any::<Vec<u8>>()) {
          let sig = key.sign(msg.as_slice());

          let msg_keys = [(msg.as_slice(), &key.pk)];

          let res = sig.aggregate_verify(&mut msg_keys.into_iter());

          assert_eq!(res, Ok(true));
      }

      #[test]
      fn verify_aggregate_signature(
          fst_key in BlsKey::arb(),
          snd_key in BlsKey::arb(),
          fst_msg in any::<Vec<u8>>(),
          snd_msg in any::<Vec<u8>>(),
      ) {
          let sig1 = fst_key.sign(fst_msg.as_slice());
          let sig2 = fst_key.sign(snd_msg.as_slice());
          let sig3 = snd_key.sign(fst_msg.as_slice());
          let sig4 = snd_key.sign(snd_msg.as_slice());

          let sig = BlsSignature::aggregate_sigs(&[&sig1, &sig2, &sig3, &sig4])
              .expect("aggregation should work");

          let msg_keys = [
              (fst_msg.as_slice(), &fst_key.pk),
              (snd_msg.as_slice(), &fst_key.pk),
              (fst_msg.as_slice(), &snd_key.pk),
              (snd_msg.as_slice(), &snd_key.pk),
          ];

          let res = sig.aggregate_verify(&mut msg_keys.into_iter());

          assert_eq!(res, Ok(true));
      }

      #[test]
      fn verify_signature_fails_with_wrong_pk(
          signing_key in BlsKey::arb(),
          other_key in BlsKey::arb(),
          msg in any::<Vec<u8>>()
      ) {
          let sig = signing_key.sign(msg.as_slice());

          let msg_keys = [(msg.as_slice(), &other_key.pk)];

          let res = sig.aggregate_verify(&mut msg_keys.into_iter());

          assert_eq!(res, Ok(false));
      }
    }
}
