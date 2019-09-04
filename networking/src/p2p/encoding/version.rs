use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};

#[derive(Serialize, Deserialize, Debug)]
pub struct Version {
    name: String,
    major: u16,
    minor: u16,
}

impl Version {
    pub fn new(name: String, major: u16, minor: u16,) -> Self {
        Version { name, major, minor }
    }
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
