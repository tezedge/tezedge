use honggfuzz::fuzz;
use log::debug;

use networking::p2p::binary_message::BinaryMessage;
use networking::p2p::encoding::prelude::*;

fn main() {
    loop {
        fuzz!(|data: &[u8]| {
            if let Err(e) = ConnectionMessage::from_bytes(data.to_vec()) {
                debug!("ConnectionMessage::from_bytes produced error for input: {:?}\nError:\n{:?}", data, e);
            }
        });
    }
}
