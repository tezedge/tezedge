// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, ProtocolHash};
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolMessage {
    protocol: Protocol,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(ProtocolMessage, body);
has_encoding!(ProtocolMessage, PROTOCOL_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::Protocol, Protocol::encoding().clone())
        ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Component {
    name: String,
    interface: Option<String>,
    implementation: String,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(Component, body);
has_encoding!(Component, COMPONENT_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::Name, Encoding::String),
            Field::new(FieldName::Interface, Encoding::option_field(Encoding::String)),
            Field::new(FieldName::Implementation, Encoding::String),
        ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Protocol {
    expected_env_version: i16,
    components: Vec<Component>,

    #[serde(skip_serializing)]
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
has_encoding!(Protocol, PROTOCOL_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::ExpectedEnvVersion, Encoding::Int16),
            Field::new(FieldName::Components, Encoding::dynamic(Encoding::list(Component::encoding().clone())))
        ])
});

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetProtocolsMessage {
    get_protocols: Vec<ProtocolHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(GetProtocolsMessage, body);
has_encoding!(GetProtocolsMessage, GET_PROTOCOLS_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::GetProtocols, Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::ProtocolHash)))),
        ])
});