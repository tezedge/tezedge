use crypto::base58::ToBase58Check;

pub mod prefix {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
}

#[derive(Debug, Clone)]
pub struct HashEncoding {
    bytes_size: usize,
    prefix: Vec<u8>,
}

/// This is hash configuration used to encode/decode data.
impl HashEncoding {
    pub fn new(bytes_size: usize, prefix: &[u8]) -> HashEncoding {
        HashEncoding { bytes_size, prefix: prefix.to_vec() }
    }

    /// Get length of hash in bytes (excluding prefix).
    pub fn get_bytes_size(&self) -> usize {
        self.bytes_size
    }
    /// Get hash prefix bytes. Prefix is used when hash is by base58check.
    pub fn get_prefix(&self) -> &Vec<u8> {
        &self.prefix
    }

    pub fn encode_bytes(&self, bytes: &[u8]) -> String {
        let mut hash = vec![];
        hash.extend_from_slice(&self.prefix);
        hash.extend(bytes);
        hash.to_base58check()
    }
}

pub fn to_prefixed_hash(prefix: &[u8], data: &[u8]) -> String {
    let mut hash = vec![];
    hash.extend_from_slice(&prefix);
    hash.extend_from_slice(&data);
    hash.to_base58check()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_chain_id() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(&prefix::CHAIN_ID, &hex::decode("8eceda2f")?);
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(&prefix::BLOCK_HASH, &hex::decode("46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d")?);
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(&prefix::CONTEXT_HASH, &hex::decode("934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9")?);
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), failure::Error> {
        let decoded = to_prefixed_hash(&prefix::OPERATION_LIST_LIST_HASH, &hex::decode("acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149")?);
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }
}