use failure::Error;

use networking::p2p::encoding::prelude::*;
use networking::p2p::binary_message::BinaryMessage;

#[test]
fn can_serialize_mempool() -> Result<(), Error> {
    let message = Mempool { known_valid: Vec::new(), pending: Vec::new(), body: Default::default() };
    let serialized = hex::encode(message.as_bytes()?);
    let expected = "000000000000000400000000";
    Ok(assert_eq!(expected, &serialized))
}