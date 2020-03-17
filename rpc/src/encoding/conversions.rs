use std::convert::From;

use failure::Fail;
use hex::FromHexError;

use crypto::hash::{ChainId, HashType};
use crypto::blake2b;
use crypto::base58::FromBase58CheckError;
use storage::context_storage::ContractAddress;

#[derive(Debug, Fail, PartialEq)]
pub enum ConversionError {
    #[fail(display = "Invalid contract id: {}", contract_id)]
    InvalidContractId {
        contract_id: String
    },

    #[fail(display = "Conversion from invalid public key")]
    InvalidPublicKey,

    #[fail(display = "Invalid hash: {}", hash)]
    InvalidHash {
        hash: String
    },

    #[fail(display = "Invalid curve tag: {}", curve_tag)]
    InvalidCurveTag {
        curve_tag: String
    },
}

impl From<hex::FromHexError> for ConversionError {
    fn from(error: FromHexError) -> Self {
        ConversionError::InvalidContractId { contract_id: error.to_string() }
    }
}

impl From<FromBase58CheckError> for ConversionError {
    fn from(error: FromBase58CheckError) -> Self {
        ConversionError::InvalidContractId { contract_id: error.to_string() }
    }
}

/// convert contract id to contract address
/// 
/// # Arguments
/// 
/// * `contract_id` - contract id (tz... or KT1...)
#[inline]
pub fn contract_id_to_address(contract_id: &str) -> Result<ContractAddress, ConversionError> {
    let contract_address = {
        if contract_id.len() == 44 {
            hex::decode(contract_id)?
        } else if contract_id.len() > 3 {
            let mut contract_address = Vec::with_capacity(22);
            match &contract_id[0..3] {
                "tz1" => {
                    contract_address.extend(&[0, 0]);
                    contract_address.extend(&HashType::ContractTz1Hash.string_to_bytes(contract_id)?);
                }
                "tz2" => {
                    contract_address.extend(&[0, 1]);
                    contract_address.extend(&HashType::ContractTz2Hash.string_to_bytes(contract_id)?);
                }
                "tz3" => {
                    contract_address.extend(&[0, 2]);
                    contract_address.extend(&HashType::ContractTz3Hash.string_to_bytes(contract_id)?);
                }
                "KT1" => {
                    contract_address.push(1);
                    contract_address.extend(&HashType::ContractKt1Hash.string_to_bytes(contract_id)?);
                    contract_address.push(0);
                }
                _ => return Err(ConversionError::InvalidContractId{contract_id: contract_id.to_string()})
            }
            contract_address
        } else {
            return Err(ConversionError::InvalidContractId{contract_id: contract_id.to_string()})
        }
    };

    Ok(contract_address)
}

/// convert public key byte string to contract id
/// 
/// # Arguments
/// 
/// * `pk` - public key in byte string format
#[inline]
pub fn public_key_to_contract_id(pk: Vec<u8>) -> Result<String, ConversionError> {
    // 1 byte tag and - 32 bytes for ed25519 (tz1)
    //                - 33 bytes for secp256k1 (tz2) and p256 (tz3)
    if pk.len() == 33 || pk.len() == 34 {
        let tag = pk[0];
        let hash = blake2b::digest_160(&pk[1..]);

        let contract_id = match tag {
            0 => {
                HashType::ContractTz1Hash.bytes_to_string(&hash)
            }
            1 => {
                HashType::ContractTz2Hash.bytes_to_string(&hash)
            }
            2 => {
                HashType::ContractTz3Hash.bytes_to_string(&hash)
            }
            _ => return Err(ConversionError::InvalidPublicKey)
        };
        Ok(contract_id)
    } else {
        return Err(ConversionError::InvalidPublicKey)
    }
}

#[inline]
pub fn hash_to_contract_id(hash: &str, curve: &str) -> Result<String, ConversionError>{
    if hash.len() == 40 {
        let contract_id = match curve {
            "ed25519" => {
                HashType::ContractTz1Hash.bytes_to_string(&hex::decode(&hash)?)
            }
            "secp256k1" => {
                HashType::ContractTz2Hash.bytes_to_string(&hex::decode(&hash)?)
            }
            "p256" => {
                HashType::ContractTz3Hash.bytes_to_string(&hex::decode(&hash)?)
            }
            _ => return Err(ConversionError::InvalidCurveTag{curve_tag: curve.to_string()})
        };
        Ok(contract_id)
    } else {
        return Err(ConversionError::InvalidHash{hash: hash.to_string()})
        }
}

#[inline]
pub fn chain_id_to_string(chain_id: &ChainId) -> String {
    HashType::ChainId.bytes_to_string(chain_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_id_to_address() -> Result<(), failure::Error> {
        let result = contract_id_to_address("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?;
        assert_eq!(result, hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?);

        let result = contract_id_to_address("tz1Y68Da76MHixYhJhyU36bVh7a8C9UmtvrR")?;
        assert_eq!(result, hex::decode("00008890efbd6ca6bbd7771c116111a2eec4169e0ed8")?);

        let result = contract_id_to_address("tz2LBtbMMvvguWQupgEmtfjtXy77cHgdr5TE")?;
        assert_eq!(result, hex::decode("0001823dd85cdf26e43689568436e43c20cc7c89dcb4")?);

        let result = contract_id_to_address("tz3e75hU4EhDU3ukyJueh5v6UvEHzGwkg3yC")?;
        assert_eq!(result, hex::decode("0002c2fe98642abd0b7dd4bc0fc42e0a5f7c87ba56fc")?);

        let result = contract_id_to_address("KT1NrjjM791v7cyo6VGy7rrzB3Dg3p1mQki3")?;
        assert_eq!(result, hex::decode("019c96e27f418b5db7c301147b3e941b41bd224fe400")?);

        let result = contract_id_to_address("tz2BFE2MEHhphgcR7demCGQP2k1zG1iMj1oj")?;
        print!("{}", hex::encode(result));
        // assert_eq!(result, hex::decode("019c96e27f418b5db7c301147b3e941b41bd224fe400")?);

        //tz2BFE2MEHhphgcR7demCGQP2k1zG1iMj1oj
        Ok(())
    }

    #[test]
    fn test_hash_to_contract_id() -> Result<(), failure::Error> {
        let result = hash_to_contract_id(&"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618", &"ed25519")?;
        assert_eq!(result, "tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17");
        
        let result = hash_to_contract_id(&"20262e6195b91181f1713c4237c8195096b8adc9", &"secp256k1")?;
        assert_eq!(result, "tz2BFE2MEHhphgcR7demCGQP2k1zG1iMj1oj");

        let result = hash_to_contract_id(&"6fde46af0356a0476dae4e4600172dc9309b3aa4", &"p256")?;
        assert_eq!(result, "tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5");

        let result = hash_to_contract_id(&"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618", &"invalidcurvetag");
        assert_eq!(result, Err(ConversionError::InvalidCurveTag{curve_tag: "invalidcurvetag".to_string()}));

        let result = hash_to_contract_id(&"2cca28a6f4ce4924cad67a2dc6618", &"ed25519");
        assert_eq!(result, Err(ConversionError::InvalidHash{hash: "2cca28a6f4ce4924cad67a2dc6618".to_string()}));

        Ok(())
    }

    #[test]
    fn test_public_key_to_contract_id() -> Result<(), failure::Error> {
        let valid_pk = vec![0,3,65,14,206,174,244,127,36,48,150,156,243,27,213,139,41,30,231,173,127,97,192,177,142,31,107,197,219,246,111,155,121];
        let short_pk = vec![0,3,65,14,206,174,244,127,36,48,150,156,243,27,213,139,41,30,231,173,127,97,192,177,142,31,107,197,219];
        let wrong_tag_pk = vec![4,3,65,14,206,174,244,127,36,48,150,156,243,27,213,139,41,30,231,173,127,97,192,177,142,31,107,197,219,246,111,155,121];

        let result = public_key_to_contract_id(valid_pk)?;
        assert_eq!(result, "tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17");

        let result = public_key_to_contract_id(short_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        let result = public_key_to_contract_id(wrong_tag_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        Ok(())
    }
}