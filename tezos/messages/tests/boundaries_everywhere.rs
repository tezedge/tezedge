// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;

use tezos_encoding::encoding::{Encoding, HasEncoding};
use tezos_messages::p2p::encoding::{
    ack::AckMessage, connection::ConnectionMessage, metadata::MetadataMessage,
    peer::PeerMessageResponse,
};

fn assert_bounded(encoding: &Encoding, unbounded: &mut HashSet<String>) -> bool {
    match encoding {
        Encoding::Unit
        | Encoding::Int8
        | Encoding::Uint8
        | Encoding::Int16
        | Encoding::Uint16
        | Encoding::Int31
        | Encoding::Int32
        | Encoding::Uint32
        | Encoding::Int64
        | Encoding::Bool
        | Encoding::Float
        | Encoding::RangedInt
        | Encoding::RangedFloat => true,

        Encoding::Z | Encoding::Mutez => false,

        Encoding::Enum => true,
        Encoding::String => false,
        Encoding::BoundedString(_) => true,
        Encoding::Bytes => false,
        Encoding::List(encoding) => {
            assert_bounded(encoding, unbounded);
            false
        }
        Encoding::BoundedList(_, encoding) => assert_bounded(encoding, unbounded),
        Encoding::Option(encoding) => assert_bounded(encoding, unbounded),
        Encoding::OptionalField(encoding) => assert_bounded(encoding, unbounded),
        Encoding::Dynamic(encoding) => assert_bounded(encoding, unbounded),
        Encoding::BoundedDynamic(_, encoding) => {
            assert_bounded(encoding, unbounded);
            true
        }
        Encoding::Sized(_, encoding) => {
            assert_bounded(encoding, unbounded);
            true
        }
        Encoding::Bounded(_, encoding) => {
            assert_bounded(encoding, unbounded);
            true
        }
        Encoding::Tup(encodings) => encodings
            .into_iter()
            .fold(true, |b, encoding| assert_bounded(encoding, unbounded) && b),

        Encoding::Obj(ty, fields) => fields.iter().fold(true, |b, field| {
            if assert_bounded(field.get_encoding(), unbounded) {
                b
            } else {
                unbounded.insert(format!("{}::{}", ty, field.get_name()));
                false
            }
        }),
        Encoding::Tags(_, tags) => tags.tags().fold(true, |b, tag| {
            if assert_bounded(tag.get_encoding(), unbounded) {
                b
            } else {
                unbounded.insert(format!("{}", tag.get_variant()));
                false
            }
        }),

        Encoding::Hash(_) => true,
        Encoding::Timestamp => true,
        Encoding::Custom => true,
        _ => unimplemented!(),
    }
}

#[test]
fn boundaries() {
    let mut unbounded = HashSet::new();

    assert_bounded(ConnectionMessage::encoding(), &mut unbounded);
    assert_bounded(MetadataMessage::encoding(), &mut unbounded);
    assert_bounded(AckMessage::encoding(), &mut unbounded);
    assert_bounded(PeerMessageResponse::encoding(), &mut unbounded);

    assert!(
        unbounded.is_empty(),
        "Unbounded fields/tags found:\n{}\n",
        unbounded.into_iter().collect::<Vec<_>>().join("\n")
    );
}
