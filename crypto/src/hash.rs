// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::base58::{FromBase58Check, FromBase58CheckError, ToBase58Check};

mod prefix_bytes {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const BLOCK_METADATA_HASH: [u8; 2] = [234, 249];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
    pub const OPERATION_METADATA_HASH: [u8; 2] = [005, 183];
    pub const OPERATION_METADATA_LIST_LIST_HASH: [u8; 3] = [29, 159, 182];
    pub const PROTOCOL_HASH: [u8; 2] = [2, 170];
    pub const CRYPTOBOX_PUBLIC_KEY_HASH: [u8; 2] = [153, 103];
    pub const CONTRACT_KT1_HASH: [u8; 3] = [2, 90, 121];
    pub const CONTRACT_TZ1_HASH: [u8; 3] = [6, 161, 159];
    pub const CONTRACT_TZ2_HASH: [u8; 3] = [6, 161, 161];
    pub const CONTRACT_TZ3_HASH: [u8; 3] = [6, 161, 164];
    pub const PUBLIC_KEY_ED25519: [u8; 4] = [13, 15, 37, 217];
    pub const PUBLIC_KEY_SECP256K1: [u8; 4] = [3, 254, 226, 86];
    pub const PUBLIC_KEY_P256: [u8; 4] = [3, 178, 139, 127];
}

pub type Hash = Vec<u8>;
pub type ChainId = Hash;
pub type BlockHash = Hash;
pub type BlockMetadataHash = Hash;
pub type OperationHash = Hash;
pub type OperationListListHash = Hash;
pub type OperationMetadataHash = Hash;
pub type OperationMetadataListListHash = Hash;
pub type ContextHash = Hash;
pub type ProtocolHash = Hash;
pub type ContractTz1Hash = Hash;
pub type ContractTz2Hash = Hash;
pub type ContractTz3Hash = Hash;
pub type CryptoboxPublicKeyHash = Hash;
pub type PublicKeyEd25519 = Hash;
pub type PublicKeySecp256k1 = Hash;
pub type PublicKeyP256 = Hash;

/// Note: see Tezos ocaml lib_crypto/base58.ml
#[derive(Debug, Copy, Clone)]
pub enum HashType {
    // "\087\082\000" (* Net(15) *)
    ChainId,
    // "\001\052" (* B(51) *)
    BlockHash,
    // "\234\249" (* bm(52) *)
    BlockMetadataHash,
    // "\002\170" (* P(51) *)
    ProtocolHash,
    // "\079\199" (* Co(52) *)
    ContextHash,
    // "\005\116" (* o(51) *)
    OperationHash,
    // "\029\159\109" (* LLo(53) *)
    OperationListListHash,
    // "\005\183" (* r(51) *)
    OperationMetadataHash,
    // "\029\159\182" (* LLr(53) *)
    OperationMetadataListListHash,
    // "\153\103" (* id(30) *)
    CryptoboxPublicKeyHash,
    // "\002\090\121" (* KT1(36) *)
    ContractKt1Hash,
    // "\006\161\159" (* tz1(36) *)
    ContractTz1Hash,
    // "\006\161\161" (* tz2(36) *)
    ContractTz2Hash,
    // "\006\161\164" (* tz3(36) *)
    ContractTz3Hash,
    // "\013\015\037\217" (* edpk(54) *)
    PublicKeyEd25519,
    // "\003\254\226\086" (* sppk(55) *)
    PublicKeySecp256k1,
    // "\003\178\139\127" (* p2pk(55) *)
    PublicKeyP256,
}

impl HashType {
    #[inline]
    pub fn base58check_prefix(&self) -> &'static [u8] {
        use prefix_bytes::*;
        match self {
            HashType::ChainId => &CHAIN_ID,
            HashType::BlockHash => &BLOCK_HASH,
            HashType::BlockMetadataHash => &BLOCK_METADATA_HASH,
            HashType::ContextHash => &CONTEXT_HASH,
            HashType::ProtocolHash => &PROTOCOL_HASH,
            HashType::OperationHash => &OPERATION_HASH,
            HashType::OperationListListHash => &OPERATION_LIST_LIST_HASH,
            HashType::OperationMetadataHash => &OPERATION_METADATA_HASH,
            HashType::OperationMetadataListListHash => &OPERATION_METADATA_LIST_LIST_HASH,
            HashType::CryptoboxPublicKeyHash => &CRYPTOBOX_PUBLIC_KEY_HASH,
            HashType::ContractKt1Hash => &CONTRACT_KT1_HASH,
            HashType::ContractTz1Hash => &CONTRACT_TZ1_HASH,
            HashType::ContractTz2Hash => &CONTRACT_TZ2_HASH,
            HashType::ContractTz3Hash => &CONTRACT_TZ3_HASH,
            HashType::PublicKeyEd25519 => &PUBLIC_KEY_ED25519,
            HashType::PublicKeySecp256k1 => &PUBLIC_KEY_SECP256K1,
            HashType::PublicKeyP256 => &PUBLIC_KEY_P256,
        }
    }

    /// Size of hash in bytes
    pub const fn size(&self) -> usize {
        match self {
            HashType::ChainId => 4,
            HashType::BlockHash
            | HashType::BlockMetadataHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash
            | HashType::OperationMetadataHash
            | HashType::OperationMetadataListListHash
            | HashType::PublicKeyEd25519 => 32,
            HashType::CryptoboxPublicKeyHash => 16,
            HashType::ContractKt1Hash
            | HashType::ContractTz1Hash
            | HashType::ContractTz2Hash
            | HashType::ContractTz3Hash => 20,
            HashType::PublicKeySecp256k1 | HashType::PublicKeyP256 => 33,
        }
    }

    /// Convert hash byte representation into string.
    pub fn hash_to_b58check(&self, data: &[u8]) -> String {
        assert_eq!(
            self.size(),
            data.len(),
            "Expected data length is {} but instead found {}",
            self.size(),
            data.len()
        );
        let mut hash = Vec::with_capacity(self.base58check_prefix().len() + data.len());
        hash.extend(self.base58check_prefix());
        hash.extend(data);
        hash.to_base58check()
    }

    /// Convert string representation of the hash to bytes form.
    pub fn b58check_to_hash(&self, data: &str) -> Result<Hash, FromBase58CheckError> {
        let mut hash = data.from_base58check()?;
        let expected_len = self.size() + self.base58check_prefix().len();
        assert_eq!(
            expected_len,
            hash.len(),
            "Expected decoded length is {} but instead found {}",
            expected_len,
            hash.len()
        );
        // prefix is not present in a binary representation
        hash.drain(0..self.base58check_prefix().len());
        Ok(hash)
    }
}

#[inline]
pub fn chain_id_to_b58_string(chain_id: &ChainId) -> String {
    HashType::ChainId.hash_to_b58check(chain_id)
}

/// Implementation of chain_id.ml -> of_block_hash
#[inline]
pub fn chain_id_from_block_hash(block_hash: &BlockHash) -> ChainId {
    let result = crate::blake2b::digest_256(block_hash);
    result[0..4].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto_box::PublicKey;
    use hex::FromHex;

    #[test]
    fn test_encode_chain_id() -> Result<(), failure::Error> {
        let decoded = HashType::ChainId.hash_to_b58check(&hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_chain_id_to_b58_string() -> Result<(), failure::Error> {
        let decoded = chain_id_to_b58_string(&hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_chain_id_from_block_hash() -> Result<(), failure::Error> {
        let decoded_chain_id: ChainId = chain_id_from_block_hash(
            &HashType::BlockHash
                .b58check_to_hash("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?,
        );
        let decoded_chain_id: &str = &chain_id_to_b58_string(&decoded_chain_id);
        let expected_chain_id = "NetXgtSLGNJvNye";
        assert_eq!(expected_chain_id, decoded_chain_id);

        let decoded_chain_id: ChainId = chain_id_from_block_hash(
            &HashType::BlockHash
                .b58check_to_hash("BLockGenesisGenesisGenesisGenesisGenesisd6f5afWyME7")?,
        );
        let decoded_chain_id: &str = &chain_id_to_b58_string(&decoded_chain_id);
        let expected_chain_id = "NetXjD3HPJJjmcd";
        assert_eq!(expected_chain_id, decoded_chain_id);

        Ok(())
    }

    #[test]
    fn test_encode_block_header_genesis() -> Result<(), failure::Error> {
        let decoded = HashType::BlockHash.hash_to_b58check(&hex::decode(
            "8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d",
        )?);
        let expected = "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), failure::Error> {
        let decoded = HashType::BlockHash.hash_to_b58check(&hex::decode(
            "46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d",
        )?);
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), failure::Error> {
        let decoded = HashType::ContextHash.hash_to_b58check(&hex::decode(
            "934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9",
        )?);
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationListListHash.hash_to_b58check(&hex::decode(
            "acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149",
        )?);
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_public_key_hash() -> Result<(), failure::Error> {
        let pk = PublicKey::from_hex(
            "2cc1b580f4b8b1f6dbd0aa1d9cde2655c2081c07d7e61249aad8b11d954fb01a",
        )?;
        let pk_hash = pk.public_key_hash();
        let decoded = HashType::CryptoboxPublicKeyHash.hash_to_b58check(pk_hash.as_ref());
        let expected = "idsg2wkkDDv2cbEMK4zH49fjgyn7XT";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz1() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz1Hash
            .hash_to_b58check(&hex::decode("83846eddd5d3c5ed96e962506253958649c84a74")?);
        let expected = "tz1XdRrrqrMfsFKA8iuw53xHzug9ipr6MuHq";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz2() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz2Hash
            .hash_to_b58check(&hex::decode("2fcb1d9307f0b1f94c048ff586c09f46614c7e90")?);
        let expected = "tz2Cfwk4ortcaqAGcVJKSxLiAdcFxXBLBoyY";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz3() -> Result<(), failure::Error> {
        let decoded = HashType::ContractTz3Hash
            .hash_to_b58check(&hex::decode("193b2b3f6b8f8e1e6b39b4d442fc2b432f6427a8")?);
        let expected = "tz3NdTPb3Ax2rVW2Kq9QEdzfYFkRwhrQRPhX";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_kt1() -> Result<(), failure::Error> {
        let decoded = HashType::ContractKt1Hash
            .hash_to_b58check(&hex::decode("42b419240509ddacd12839700b7f720b4aa55e4e")?);
        let expected = "KT1EfTusMLoeCAAGd9MZJn5yKzFr6kJU5U91";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_block_header_hash() -> Result<(), failure::Error> {
        let decoded = HashType::BlockHash
            .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
        let decoded = hex::encode(&decoded);
        let expected = "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationListListHash
            .b58check_to_hash("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?;
        let decoded = hex::encode(&decoded);
        let expected = "7c09f7c4d76ace86e1a7e1c7dc0a0c7edcaa8b284949320081131976a87760c3";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_protocol_hash() -> Result<(), failure::Error> {
        let decoded = HashType::ProtocolHash
            .b58check_to_hash("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;
        let decoded = hex::encode(&decoded);
        let expected = "3e5e3a606afab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb";
        assert_eq!(expected, decoded);

        assert_eq!(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
            HashType::ProtocolHash.hash_to_b58check(&hex::decode(decoded)?)
        );
        Ok(())
    }

    #[test]
    fn test_decode_block_metadata_hash() -> Result<(), failure::Error> {
        let decoded = HashType::BlockMetadataHash
            .b58check_to_hash("bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN")?;
        let decoded = hex::encode(&decoded);
        let expected = "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8";
        assert_eq!(expected, decoded);

        assert_eq!(
            "bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN",
            HashType::BlockMetadataHash.hash_to_b58check(&hex::decode(decoded)?)
        );
        Ok(())
    }

    #[test]
    fn test_decode_operation_metadata_list_list_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationMetadataListListHash
            .b58check_to_hash("LLr283rR7AWhepNeHcP9msa2VeAurWtodBLrnSjwaxpNyiyfhYcKX")?;
        let decoded = hex::encode(&decoded);
        let expected = "761223dee6643bb9f28acf45f2a44ae1a3e2fd68bfc1a00e3f539cc2dc637632";
        assert_eq!(expected, decoded);

        assert_eq!(
            "LLr283rR7AWhepNeHcP9msa2VeAurWtodBLrnSjwaxpNyiyfhYcKX",
            HashType::OperationMetadataListListHash.hash_to_b58check(&hex::decode(decoded)?)
        );
        Ok(())
    }

    #[test]
    fn test_decode_operation_metadata_hash() -> Result<(), failure::Error> {
        let decoded = HashType::OperationMetadataHash
            .b58check_to_hash("r3E9xb2QxUeG56eujC66B56CV8mpwjwfdVmEpYu3FRtuEx9tyfG")?;
        let decoded = hex::encode(&decoded);
        let expected = "2d905a5c4fefad1f1ab8a5436f26b15290f2fe2bea111c85ee4626156dc6b4da";
        assert_eq!(expected, decoded);

        assert_eq!(
            "r3E9xb2QxUeG56eujC66B56CV8mpwjwfdVmEpYu3FRtuEx9tyfG",
            HashType::OperationMetadataHash.hash_to_b58check(&hex::decode(decoded)?)
        );
        Ok(())
    }
}
