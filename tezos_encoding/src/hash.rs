use crypto::base58::ToBase58Check;

mod prefix_bytes {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
}

pub type Hash = Vec<u8>;
pub type ChainId = Hash;
pub type BlockHash = Hash;
pub type OperationHash = Hash;
pub type OperationListListHash = Hash;
pub type ContextHash = Hash;

#[derive(Debug, Copy, Clone)]
pub enum HashType {
    ChainId,
    BlockHash,
    ContextHash,
    OperationHash,
    OperationListListHash,
}

impl HashType {
    pub fn prefix(&self) -> &'static [u8] {
        use prefix_bytes::*;
        match self {
            HashType::ChainId => &CHAIN_ID,
            HashType::BlockHash => &BLOCK_HASH,
            HashType::ContextHash => &CONTEXT_HASH,
            HashType::OperationHash => &OPERATION_HASH,
            HashType::OperationListListHash => &OPERATION_LIST_LIST_HASH,
        }
    }

    pub fn size(&self) -> usize {
        match self {
            HashType::ChainId => 4,
            HashType::BlockHash
            | HashType::ContextHash
            | HashType::OperationHash
            | HashType::OperationListListHash => 32,
        }
    }
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

    pub fn encode_bytes(&self, bytes: &[u8]) -> String {
        to_prefixed_hash(self.0.prefix(), bytes)
    }
}

pub fn to_prefixed_hash(prefix: &[u8], data: &[u8]) -> String {
    let mut hash = vec![];
    hash.extend(prefix);
    hash.extend(data);
    hash.to_base58check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_chain_id() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(HashType::ChainId.prefix(), &hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(HashType::BlockHash.prefix(), &hex::decode("46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d")?);
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(HashType::ContextHash.prefix(), &hex::decode("934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9")?);
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(HashType::OperationListListHash.prefix(), &hex::decode("acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149")?);
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }
}