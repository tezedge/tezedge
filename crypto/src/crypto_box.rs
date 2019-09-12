use failure::Fail;
use hex::{FromHex, FromHexError};
use sodiumoxide::crypto::box_;

use super::nonce::Nonce;
use std::ops::Deref;

pub const BOX_ZERO_BYTES: usize = 32;
const CRYPTO_KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 24;

#[derive(Clone, PartialEq)]
pub struct PublicKey(box_::PublicKey);

impl AsRef<box_::PublicKey> for PublicKey {
    fn as_ref(&self) -> &box_::PublicKey {
        &self.0
    }
}

impl Deref for PublicKey {
    type Target = box_::PublicKey;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromHex for PublicKey {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let mut arr = [0u8; CRYPTO_KEY_SIZE];
        arr.copy_from_slice(&bytes);
        Ok(PublicKey(box_::PublicKey(arr)))
    }
}

#[derive(Clone, PartialEq)]
pub struct SecretKey(box_::SecretKey);

impl AsRef<box_::SecretKey> for SecretKey {
    fn as_ref(&self) -> &box_::SecretKey {
        &self.0
    }
}

impl Deref for SecretKey {
    type Target = box_::SecretKey;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromHex for SecretKey {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let mut arr = [0u8; CRYPTO_KEY_SIZE];
        arr.copy_from_slice(&bytes);
        Ok(SecretKey(box_::SecretKey(arr)))
    }
}

#[derive(Clone, PartialEq)]
pub struct PrecomputedKey(box_::PrecomputedKey);

impl FromHex for PrecomputedKey {
    type Error = FromHexError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        let bytes = hex::decode(hex)?;
        let mut arr = [0u8; CRYPTO_KEY_SIZE];
        arr.copy_from_slice(&bytes);
        Ok(PrecomputedKey(box_::PrecomputedKey(arr)))
    }
}

impl AsRef<box_::PrecomputedKey> for PrecomputedKey {
    fn as_ref(&self) -> &box_::PrecomputedKey {
        &self.0
    }
}

impl Deref for PrecomputedKey {
    type Target = box_::PrecomputedKey;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}


#[derive(Debug, Fail)]
pub enum CryptoError {
    #[fail(display = "invalid nonce size: {}", _0)]
    InvalidNonceSize(usize),
    #[fail(display = "failed to decrypt")]
    FailedToDecrypt,
}

pub fn precompute(pk_as_hex_string: &str, sk_as_hex_string: &str) -> Result<PrecomputedKey, FromHexError> {
    Ok(PrecomputedKey(box_::precompute(&*PublicKey::from_hex(pk_as_hex_string)?, &*SecretKey::from_hex(sk_as_hex_string)?)))
}

pub fn encrypt(msg: &[u8], nonce: &Nonce, pck: &PrecomputedKey) -> Result<Vec<u8>, CryptoError> {
    let nonce_bytes = nonce.get_bytes();
    if nonce_bytes.len() == NONCE_SIZE {
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce_bytes);
        let box_nonce = box_::Nonce(nonce_arr);

        Ok(box_::seal_precomputed(msg, &box_nonce, &*pck))
    } else {
        Err(CryptoError::InvalidNonceSize(nonce_bytes.len()))
    }
}

pub fn decrypt(enc: &[u8], nonce: &Nonce, pck: &PrecomputedKey) -> Result<Vec<u8>, CryptoError> {
    let nonce_bytes = nonce.get_bytes();
    if nonce_bytes.len() == NONCE_SIZE {
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce_bytes);
        let box_nonce = box_::Nonce(nonce_arr);

        match box_::open_precomputed(enc, &box_nonce, pck) {
            Ok(msg) => Ok(msg),
            Err(()) => Err(CryptoError::FailedToDecrypt)
        }
    } else {
        Err(CryptoError::InvalidNonceSize(nonce_bytes.len()))
    }
}


#[cfg(test)]
mod tests {
    use failure::Error;
    use super::*;

    #[test]
    fn generate_nonce_xsalsa20() {
        let nonce = box_::gen_nonce();
        assert_eq!(NONCE_SIZE, nonce.0.len())
    }

    #[test]
    fn generate_precomputed_key() -> Result<(), Error> {
        let pk = "96678b88756dd6cfd6c129980247b70a6e44da77823c3672a2ec0eae870d8646";
        let sk = "a18dc11cb480ebd31081e1541df8bd70c57da0fa419b5036242f8619d605e75a";

        let precomputed = precompute(&pk, &sk).unwrap();
        let precomputed = hex::encode(&precomputed.0);
        let expected_precomputed = "5228751a6f5a6494e38e1042f578e3a64ae3462b7899356f49e50be846c9609c";
        Ok(assert_eq!(expected_precomputed, precomputed))
    }

    #[test]
    fn encrypt_message() -> Result<(), Error> {
        let nonce = Nonce::new(&hex::decode("8dde158c55cff52f4be9352787d333e616a67853640d72c5")?);
        let msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000")?;
        let pck = PrecomputedKey::from_hex("5228751a6f5a6494e38e1042f578e3a64ae3462b7899356f49e50be846c9609c")?;

        let encrypted_msg = encrypt(&msg, &nonce, &pck)?;
        let expected_encrypted_msg = hex::decode("45d82d5c4067f5c32748596c1bbc93a9f87b5b1f2058ddd82b6f081ca484b672395c7473ab897c64c01c33878ac1ccb6919a75c9938d8bcf0e7917ddac13a787cfb5c9a5aea50d24502cf86b5c9b000358c039334ec077afe98936feec0dabfff35f14cafd2cd3173bbd56a7c6e5bf6f5f57c92b59b129918a5895e883e7d999b191aad078c4a5b164144c1beaed58b49ba9be094abf3a3bd9")?;
        Ok(assert_eq!(expected_encrypted_msg, encrypted_msg))
    }

    #[test]
    fn decrypt_message() -> Result<(), Error> {
        let nonce = Nonce::new(&hex::decode("8dde158c55cff52f4be9352787d333e616a67853640d72c5")?);
        let enc = hex::decode("45d82d5c4067f5c32748596c1bbc93a9f87b5b1f2058ddd82b6f081ca484b672395c7473ab897c64c01c33878ac1ccb6919a75c9938d8bcf0e7917ddac13a787cfb5c9a5aea50d24502cf86b5c9b000358c039334ec077afe98936feec0dabfff35f14cafd2cd3173bbd56a7c6e5bf6f5f57c92b59b129918a5895e883e7d999b191aad078c4a5b164144c1beaed58b49ba9be094abf3a3bd9")?;
        let pck = PrecomputedKey::from_hex("5228751a6f5a6494e38e1042f578e3a64ae3462b7899356f49e50be846c9609c")?;

        let decrypted_msg = decrypt(&enc, &nonce, &pck)?;
        let expected_decrypted_msg = hex::decode("00874d1b98317bd6efad8352a7144c9eb0b218c9130e0a875973908ddc894b764ffc0d7f176cf800b978af9e919bdc35122585168475096d0ebcaca1f2a1172412b91b363ff484d1c64c03417e0e755e696c386a0000002d53414e44424f5845445f54455a4f535f414c5048414e45545f323031382d31312d33305431353a33303a35365a00000000")?;
        Ok(assert_eq!(expected_decrypted_msg, decrypted_msg))
    }

    #[test]
    fn decryption_of_encrypted_should_equal_message() -> Result<(), Error> {
        let nonce = Nonce::new(&hex::decode("8dde158c55cff52f4be9352787d333e616a67853640d72c5")?);
        let msg = "hello world";
        let pck = PrecomputedKey::from_hex("5228751a6f5a6494e38e1042f578e3a64ae3462b7899356f49e50be846c9609c")?;

        let enc = encrypt(msg.as_bytes(), &nonce, &pck)?;
        let dec = String::from_utf8(decrypt(&enc, &nonce, &pck).unwrap())?;
        Ok(assert_eq!(msg, &dec))
    }
}