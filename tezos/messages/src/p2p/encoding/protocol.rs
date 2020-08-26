// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, ProtocolHash};
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolMessage {
    protocol: Protocol,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for ProtocolMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::Protocol, Protocol::encoding())
        ])
    }
}

impl CachedData for ProtocolMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Component {
    name: String,
    interface: Option<String>,
    implementation: String,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for Component {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::Name, Encoding::String),
            Field::new(FieldName::Interface, Encoding::option_field(Encoding::String)),
            Field::new(FieldName::Implementation, Encoding::String),
        ])
    }
}

impl CachedData for Component {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

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

impl HasEncoding for Protocol {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::ExpectedEnvVersion, Encoding::Int16),
            Field::new(FieldName::Components, Encoding::dynamic(Encoding::list(Component::encoding())))
        ])
    }
}

impl CachedData for Protocol {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GetProtocolsMessage {
    get_protocols: Vec<ProtocolHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for GetProtocolsMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::GetProtocols, Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::ProtocolHash)))),
        ])
    }
}

impl CachedData for GetProtocolsMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}