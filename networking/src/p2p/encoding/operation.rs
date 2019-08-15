use std::rc::Rc;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{HashEncoding, HashType};

use super::*;

#[derive(Serialize, Deserialize, Debug)]
pub struct OperationMessage {
    operation: Operation
}

impl HasEncoding for OperationMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("operation", Operation::encoding())
        ])
    }
}


// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct Operation {
    branch: BlockHash,
    data: Vec<u8>,
}

impl HasEncoding for Operation {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("branch", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
            Field::new("data", Encoding::Split(Rc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Bytes,
                    SchemaType::Binary => Encoding::list(Encoding::Uint8)
                }
            )))
        ])
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::to_prefixed_hash;

    use crate::p2p::message::BinaryMessage;

    use super::*;

    #[test]
    fn can_deserialize() -> Result<(), Error> {
        let message_bytes = hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08")?;
        let operation = Operation::from_bytes(message_bytes)?;
//        let hash_encoding = HashEncoding::new(32, &hash::prefix::BLOCK_HASH);
        assert_eq!("BKqTKfGwK3zHnVXX33X5PPHy1FDTnbkajj3eFtCXGFyfimQhT1H", to_prefixed_hash(HashType::BlockHash.prefix(), &operation.branch));
        assert_eq!("000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08", &hex::encode(&operation.data));

        Ok(())
    }
}