// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::ProtocolHash;
use tezos_encoding::enc::BinWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::nom::NomReader;

use super::limits::{
    COMPONENT_IMPLEMENTATION_MAX_LENGTH, COMPONENT_INTERFACE_MAX_LENGTH, COMPONENT_NAME_MAX_LENGTH,
    GET_PROTOCOLS_MAX_LENGTH, PROTOCOL_COMPONENT_MAX_SIZE,
};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Eq,
    PartialEq,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct ProtocolMessage {
    protocol: Protocol,
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Eq,
    PartialEq,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct Component {
    #[encoding(string = "COMPONENT_NAME_MAX_LENGTH")]
    name: String,
    #[encoding(string = "COMPONENT_INTERFACE_MAX_LENGTH")]
    interface: Option<String>,
    #[encoding(string = "COMPONENT_IMPLEMENTATION_MAX_LENGTH")]
    implementation: String,
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Eq,
    PartialEq,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct Protocol {
    expected_env_version: i16,
    #[encoding(dynamic = "PROTOCOL_COMPONENT_MAX_SIZE", list)]
    components: Vec<Component>,
}

impl Protocol {
    pub fn expected_env_version(&self) -> i16 {
        self.expected_env_version
    }

    pub fn components(&self) -> &Vec<Component> {
        &self.components
    }
}

// -----------------------------------------------------------------------------------------------
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(
    Serialize,
    Deserialize,
    Debug,
    Eq,
    PartialEq,
    Clone,
    HasEncoding,
    NomReader,
    BinWriter,
    tezos_encoding::generator::Generated,
)]
pub struct GetProtocolsMessage {
    #[encoding(dynamic, list = "GET_PROTOCOLS_MAX_LENGTH")]
    get_protocols: Vec<ProtocolHash>,
}
