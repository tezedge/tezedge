use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{HashEncoding, HashType, ProtocolHash};

#[derive(Serialize, Deserialize, Debug)]
pub struct ProtocolMessage {
    protocol: Protocol
}

impl HasEncoding for ProtocolMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("protocol", Protocol::encoding())
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct Component {
    name: String,
    interface: String,
    implementation: String,
}

impl HasEncoding for Component {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("name", Encoding::String),
            Field::new("interface", Encoding::option(Encoding::String)),
            Field::new("implementation", Encoding::String),
        ])
    }
}


// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct Protocol {
    expected_env_version: i16,
    components: Vec<Component>,
}

impl HasEncoding for Protocol {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("expected_env_version", Encoding::Int16),
            Field::new("components", Component::encoding())
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetProtocolsMessage {
    get_protocols: Vec<ProtocolHash>,
}

impl HasEncoding for GetProtocolsMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_protocols", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::ProtocolHash))))),
        ])
    }
}
