// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::base58::{FromBase58Check, FromBase58CheckError, ToBase58Check};

mod prefix_bytes {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
    pub const PROTOCOL_HASH: [u8; 2] = [2, 170];
    pub const CRYPTOBOX_PUBLIC_KEY_HASH: [u8; 2] = [153, 103];
    pub const CONTRACT_KT1_HASH: [u8; 3] = [2, 90, 121];
    pub const CONTRACT_TZ1_HASH: [u8; 3] = [6, 161, 159];
    pub const CONTRACT_TZ2_HASH: [u8; 3] = [6, 161, 161];
    pub const CONTRACT_TZ3_HASH: [u8; 3] = [6, 161, 164];
}

pub type Hash = Vec<u8>;
pub type ChainId = Hash;
pub type BlockHash = Hash;
pub type OperationHash = Hash;
pub type OperationListListHash = Hash;
pub type ContextHash = Hash;
pub type ProtocolHash = Hash;
pub type ContractTz1Hash = Hash;
pub type ContractTz2Hash = Hash;
pub type ContractTz3Hash = Hash;
pub type CryptoboxPublicKeyHash = Hash;

#[derive(Debug, Copy, Clone)]
pub enum HashType {
    ChainId,
    // "\087\082\000" (* Net(15) *)
    BlockHash,
    // "\001\052" (* B(51) *)
    ProtocolHash,
    // "\002\170" (* P(51) *)
    ContextHash,
    // "\079\199" (* Co(52) *)
    OperationHash,
    // "\005\116" (* o(51) *)
    OperationListListHash,
    // "\029\159\109" (* LLo(53) *)
    CryptoboxPublicKeyHash,
    // "\153\103" (* id(30) *)
    ContractKt1Hash,
    // "\002\090\121" (* KT1(36) *)
    ContractTz1Hash,
    // "\006\161\159" (* tz1(36) *)
    ContractTz2Hash,
    // "\006\161\161" (* tz2(36) *)
    ContractTz3Hash,
    // "\006\161\164" (* tz3(36) *)
}

impl HashType {

    #[inline]
    pub fn prefix(&self) -> &'static [u8] {
        use prefix_bytes::*;
        match self {
            HashType::ChainId => &CHAIN_ID,
            HashType::BlockHash => &BLOCK_HASH,
            HashType::ContextHash => &CONTEXT_HASH,
            HashType::ProtocolHash => &PROTOCOL_HASH,
            HashType::OperationHash => &OPERATION_HASH,
            HashType::OperationListListHash => &OPERATION_LIST_LIST_HASH,
            HashType::CryptoboxPublicKeyHash => &CRYPTOBOX_PUBLIC_KEY_HASH,
            HashType::ContractKt1Hash => &CONTRACT_KT1_HASH,
            HashType::ContractTz1Hash => &CONTRACT_TZ1_HASH,
            HashType::ContractTz2Hash => &CONTRACT_TZ2_HASH,
            HashType::ContractTz3Hash => &CONTRACT_TZ3_HASH,
        }
    }

    /// Size of hash in bytes
    pub const fn size(&self) -> usize {
        match self {
            HashType::ChainId => 4,
            HashType::BlockHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash => 32,
            HashType::CryptoboxPublicKeyHash => 16,
            HashType::ContractKt1Hash
            | HashType::ContractTz1Hash
            | HashType::ContractTz2Hash
            | HashType::ContractTz3Hash => 20,
        }
    }

    pub const fn hash_fn<'a>(&self) -> &'a dyn Fn(&'a [u8]) -> Vec<u8> {
        match self {
            HashType::ChainId
            | HashType::BlockHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash
            | HashType::ContractKt1Hash
            | HashType::ContractTz1Hash
            | HashType::ContractTz2Hash
            | HashType::ContractTz3Hash => &copy_bytes,
            HashType::CryptoboxPublicKeyHash => &crate::blake2b::digest_128
        }
    }

    /// Convert hash byte representation into string.
    pub fn bytes_to_string(&self, data: &[u8]) -> String {
        let hash_fn = self.hash_fn();
        let data = hash_fn(data);

        assert_eq!(self.size(), data.len(), "Expected data length is {} but instead found {}", self.size(), data.len());
        let mut hash = Vec::with_capacity(self.prefix().len() + data.len());
        hash.extend(self.prefix());
        hash.extend(data);
        hash.to_base58check()
    }

    /// Convert string representation of the hash to bytes form.
    pub fn string_to_bytes(&self, data: &str) -> Result<Hash, FromBase58CheckError> {
        let mut hash = data.from_base58check()?;
        let expected_len = self.size() + self.prefix().len();
        assert_eq!(expected_len, hash.len(), "Expected decoded length is {} but instead found {}", expected_len, hash.len());
        // prefix is not present in a binary representation
        hash.drain(0..self.prefix().len());
        Ok(hash)
    }
}

// dummy hashing function
fn copy_bytes(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_chain_id() -> Result<(), failure::Error> {
        let decoded = HashType::ChainId.bytes_to_string(&hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), failure::Error> {
        let decoded = HashType::BlockHash.bytes_to_string(&hex::decode("46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d")?);
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), failure::Error> {
        let decoded = HashType::ContextHash.bytes_to_string(&hex::decode("934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9")?);
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationListListHash.bytes_to_string(&hex::decode("acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149")?);
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_public_key_hash() -> Result<(), failure::Error> {
        let decoded = HashType::CryptoboxPublicKeyHash.bytes_to_string(&hex::decode("2cc1b580f4b8b1f6dbd0aa1d9cde2655c2081c07d7e61249aad8b11d954fb01a")?);
        let expected = "idsg2wkkDDv2cbEMK4zH49fjgyn7XT";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz1() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz1Hash.bytes_to_string(&hex::decode("83846eddd5d3c5ed96e962506253958649c84a74")?);
        let expected = "tz1XdRrrqrMfsFKA8iuw53xHzug9ipr6MuHq";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz2() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz2Hash.bytes_to_string(&hex::decode("2fcb1d9307f0b1f94c048ff586c09f46614c7e90")?);
        let expected = "tz2Cfwk4ortcaqAGcVJKSxLiAdcFxXBLBoyY";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz3() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz3Hash.bytes_to_string(&hex::decode("193b2b3f6b8f8e1e6b39b4d442fc2b432f6427a8")?);
        let expected = "tz3NdTPb3Ax2rVW2Kq9QEdzfYFkRwhrQRPhX";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_kt1() -> Result<(), failure::Error> {
        let decoded = HashType::ContractKt1Hash.bytes_to_string(&hex::decode("42b419240509ddacd12839700b7f720b4aa55e4e")?);
        let expected = "KT1EfTusMLoeCAAGd9MZJn5yKzFr6kJU5U91";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_block_header_hash() -> Result<(), failure::Error> {
        let decoded = HashType::BlockHash.string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
        let decoded = hex::encode(&decoded);
        let expected = "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationListListHash.string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?;
        let decoded = hex::encode(&decoded);
        let expected = "7c09f7c4d76ace86e1a7e1c7dc0a0c7edcaa8b284949320081131976a87760c3";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_protocol_hash() -> Result<(), failure::Error> {
        let decoded = HashType::ProtocolHash.string_to_bytes("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;
        let decoded = hex::encode(&decoded);
        let expected = "3e5e3a606afab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb";
        assert_eq!(expected, decoded);

        assert_eq!("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb", HashType::ProtocolHash.bytes_to_string(&hex::decode(decoded)?));
        Ok(())
    }
}