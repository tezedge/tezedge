use crypto::base58::{FromBase58Check, FromBase58CheckError, ToBase58Check};

mod prefix_bytes {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
    pub const PROTOCOL_HASH: [u8; 2] = [2, 170];
    pub const PUBLIC_KEY_HASH: [u8; 2] = [153, 103];
}

pub type Hash = Vec<u8>;
pub type ChainId = Hash;
pub type BlockHash = Hash;
pub type OperationHash = Hash;
pub type OperationListListHash = Hash;
pub type ContextHash = Hash;
pub type ProtocolHash = Hash;

#[derive(Debug, Copy, Clone)]
pub enum HashType {
    ChainId,
    BlockHash,
    ProtocolHash,
    ContextHash,
    OperationHash,
    OperationListListHash,
    PublicKeyHash,
}

impl HashType {

    // TODO: make const after `#![feature(const_if_match)]` lands
    #[inline]
    pub fn prefix(self) -> &'static [u8] {
        use prefix_bytes::*;
        match self {
            HashType::ChainId => &CHAIN_ID,
            HashType::BlockHash => &BLOCK_HASH,
            HashType::ContextHash => &CONTEXT_HASH,
            HashType::ProtocolHash => &PROTOCOL_HASH,
            HashType::OperationHash => &OPERATION_HASH,
            HashType::OperationListListHash => &OPERATION_LIST_LIST_HASH,
            HashType::PublicKeyHash => &PUBLIC_KEY_HASH,
        }
    }

    /// Size of hash in bytes
    #[inline]
    pub fn size(self) -> usize {
        match self {
            HashType::ChainId => 4,
            HashType::BlockHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash => 32,
            HashType::PublicKeyHash => 16,
        }
    }

    #[inline]
    pub fn hash_fn<'a>(self) -> &'a dyn Fn(&'a [u8]) -> Vec<u8> {
        match self {
            HashType::ChainId
            | HashType::BlockHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash => &copy_bytes,
            HashType::PublicKeyHash => &crypto::blake2b::digest_128
        }
    }
}

fn copy_bytes(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

#[derive(Debug, Clone)]
pub struct HashEncoding(HashType);

/// This is hash configuration used to encode/decode data.
impl HashEncoding {
    pub fn new(hash_type: HashType) -> HashEncoding {
        HashEncoding(hash_type)
    }

    /// Get length of hash in bytes (excluding prefix).
    pub fn get_bytes_size(&self) -> usize {
        self.0.size()
    }

    /// Convert hash byte representation into string.
    pub fn bytes_to_string(&self, data: &[u8]) -> String {
        let hash_fn = self.0.hash_fn();
        let data = hash_fn(data);

        assert_eq!(self.0.size(), data.len(), "Expected data length is {} but instead found {}", self.0.size(), data.len());
        let mut hash = vec![];
        hash.extend(self.0.prefix());
        hash.extend(data);
        hash.to_base58check()
    }

    /// Convert string representation of the hash to bytes form.
    pub fn string_to_bytes(&self, data: &str) -> Result<Hash, FromBase58CheckError> {
        let mut hash = data.from_base58check()?;
        let expected_len = self.0.size() + self.0.prefix().len();
        assert_eq!(expected_len, hash.len(), "Expected decoded length is {} but instead found {}", expected_len, hash.len());
        hash.drain(0..self.0.prefix().len());
        Ok(hash)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_chain_id() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::ChainId).bytes_to_string(&hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::BlockHash).bytes_to_string(&hex::decode("46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d")?);
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::ContextHash).bytes_to_string(&hex::decode("934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9")?);
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::OperationListListHash).bytes_to_string(&hex::decode("acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149")?);
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_public_key_hash() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::PublicKeyHash).bytes_to_string(&hex::decode("2cc1b580f4b8b1f6dbd0aa1d9cde2655c2081c07d7e61249aad8b11d954fb01a")?);
        let expected = "idsg2wkkDDv2cbEMK4zH49fjgyn7XT";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_block_header_hash() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
        let decoded = hex::encode(&decoded);
        let expected = "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_operations_hash() -> Result<(), failure::Error> {
        let decoded = HashEncoding::new(HashType::OperationListListHash).string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?;
        let decoded = hex::encode(&decoded);
        let expected = "7c09f7c4d76ace86e1a7e1c7dc0a0c7edcaa8b284949320081131976a87760c3";
        assert_eq!(expected, decoded);

        Ok(())
    }
}