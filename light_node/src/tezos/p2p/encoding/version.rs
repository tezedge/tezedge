use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

#[derive(Serialize, Deserialize, Debug)]
pub struct Version {
    name: String,
    major: u16,
    minor: u16,
}

impl HasEncoding for Version {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("name", Encoding::String),
            Field::new("major", Encoding::Uint16),
            Field::new("minor", Encoding::Uint16)
        ])
    }
}

impl From<&crate::tezos::p2p::client::Version> for Version {
    fn from(version: &crate::tezos::p2p::client::Version) -> Self {
        Version {
            name: version.name.clone(),
            major: version.major,
            minor: version.minor
        }
    }
}
