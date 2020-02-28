use failure::bail;

use crypto::hash::{ChainId, HashType};
use crypto::blake2b;
use storage::context_storage::ContractAddress;

/// convert contract id to contract address
/// 
/// # Arguments
/// 
/// * `contract_id` - contract id (tz... or KT1...)
#[inline]
pub fn contract_id_to_address(contract_id: &str) -> Result<ContractAddress, failure::Error> {
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
                _ => bail!("Invalid contract id")
            }
            contract_address
        } else {
            bail!("Invalid contract id");
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
pub fn public_key_to_contract_id(pk: Vec<u8>) -> Result<String, failure::Error> {
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
            _ => bail!("Invalid public key")
        };
        Ok(contract_id)
    } else {
        bail!("Invalid public key")
    }
}

#[inline]
pub fn hash_to_contract_id(hash: &str, curve: &str) -> Result<String, failure::Error>{
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
            _ => bail!("Invalid curve tag")
        };
        Ok(contract_id)
    } else {
        bail!("Invalid hash")
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

        Ok(())
    }
}