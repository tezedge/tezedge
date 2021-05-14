// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, ProtocolHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, HasEncodingTest};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::{GET_PROTOCOLS_MAX_LENGTH, PROTOCOL_COMPONENT_MAX_SIZE};

#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader)]
pub struct ProtocolMessage {
    protocol: Protocol,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(ProtocolMessage, body);
has_encoding_test!(ProtocolMessage, PROTOCOL_MESSAGE_ENCODING, {
    Encoding::Obj(
        "ProtocolMessage",
        vec![Field::new("protocol", Protocol::encoding_test().clone())],
    )
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader)]
pub struct Component {
    name: String,
    interface: Option<String>,
    implementation: String,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(Component, body);
has_encoding_test!(Component, COMPONENT_ENCODING, {
    Encoding::Obj(
        "Component",
        vec![
            Field::new("name", Encoding::String),
            Field::new("interface", Encoding::option_field(Encoding::String)),
            Field::new("implementation", Encoding::String),
        ],
    )
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader)]
pub struct Protocol {
    expected_env_version: i16,
    #[encoding(dynamic = "PROTOCOL_COMPONENT_MAX_SIZE", list)]
    components: Vec<Component>,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

impl Protocol {
    pub fn expected_env_version(&self) -> i16 {
        self.expected_env_version
    }

    pub fn components(&self) -> &Vec<Component> {
        &self.components
    }
}

cached_data!(Protocol, body);
has_encoding_test!(Protocol, PROTOCOL_ENCODING, {
    Encoding::Obj(
        "Protocol",
        vec![
            Field::new("expected_env_version", Encoding::Int16),
            Field::new(
                "components",
                Encoding::bounded_dynamic(
                    PROTOCOL_COMPONENT_MAX_SIZE,
                    Encoding::list(Component::encoding_test().clone()),
                ),
            ),
        ],
    )
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader)]
pub struct GetProtocolsMessage {
    #[encoding(dynamic, list = "GET_PROTOCOLS_MAX_LENGTH")]
    get_protocols: Vec<ProtocolHash>,

    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

cached_data!(GetProtocolsMessage, body);
has_encoding_test!(GetProtocolsMessage, GET_PROTOCOLS_MESSAGE_ENCODING, {
    Encoding::Obj(
        "GetProtocolsMessage",
        vec![Field::new(
            "get_protocols",
            Encoding::dynamic(Encoding::bounded_list(
                GET_PROTOCOLS_MAX_LENGTH,
                Encoding::Hash(HashType::ProtocolHash),
            )),
        )],
    )
});

#[cfg(test)]
mod test {
    use tezos_encoding::assert_encodings_match;

    use super::*;

    #[test]
    fn test_protocol_encoding_schema() {
        assert_encodings_match!(ProtocolMessage);
    }

    #[test]
    fn test_get_protocol_encoding_schema() {
        assert_encodings_match!(GetProtocolsMessage);
    }
}
