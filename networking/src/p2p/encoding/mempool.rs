use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash;
use tezos_encoding::hash::HashEncoding;

#[derive(Serialize, Deserialize, Debug)]
pub struct Mempool {
    known_valid: Vec<Vec<u8>>,
    pending: Vec<Vec<u8>>,
}

#[allow(dead_code)]
impl Mempool {
    pub fn get_known_valid(&self) -> &Vec<Vec<u8>> {
        &self.known_valid
    }

    pub fn get_pending(&self) -> &Vec<Vec<u8>> {
        &self.pending
    }
}

impl HasEncoding for Mempool {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("known_valid", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(32, &hash::prefix::OPERATION_HASH))))),
            Field::new("pending", Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(32, &hash::prefix::OPERATION_HASH)))))),
        ])
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use crate::tezos::p2p::message::BinaryMessage;

    use super::*;

    #[test]
    fn can_serialize_mempool() -> Result<(), Error> {
        let message = Mempool { pending: Vec::new(), known_valid: Vec::new() };
        let serialized = hex::encode(message.as_bytes()?);
        let expected = "000000000000000400000000";
        Ok(assert_eq!(expected, &serialized))
    }
}