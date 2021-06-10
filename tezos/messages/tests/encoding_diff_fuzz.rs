// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod encoding_diff_common;
mod message_limit;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use encoding_diff_common::*;
use serde::Serialize;
use tezos_encoding::enc::{BinError, BinErrorKind, BinWriter};
use tezos_encoding::encoding::{Encoding, HasEncoding};
use tezos_encoding::generator::{value, values, Generated, IntType};

use tezos_encoding::generator::{some_in_range, Generator, GeneratorFactory};
use tezos_messages::p2p::encoding::prelude::*;

use message_limit::*;

const MAX_LENGTH: usize = 1024 * 1024;

fn string_of_chars(chars: &str, len: usize) -> String {
    let mut result = String::new();
    while result.len() < len {
        result += chars;
    }
    result.split_at(len).0.to_string()
}

fn in_bounds_and_over<T: IntType>(upper_bound: T) -> impl Generator<Item = T> {
    some_in_range(..=upper_bound).and(some_in_range(
        upper_bound + T::one()..(upper_bound << T::one()),
    ))
}

fn get_max_element_size(encoding: &Encoding) -> Option<usize> {
    match get_max_size(encoding) {
        Limit::Fixed(size) | Limit::UpTo(size) => Some(size),
        Limit::Var => None,
    }
}

struct IteratingGeneratorFactory {
    bounds_size: Rc<RefCell<Option<usize>>>,
}

impl IteratingGeneratorFactory {
    fn new(bounds_size: Rc<RefCell<Option<usize>>>) -> Self {
        Self { bounds_size }
    }
}

macro_rules! int_factory {
    ($ty:ident) => {
        fn $ty(&mut self, _field: &str) -> Box<dyn Generator<Item = $ty>> {
            Box::new(value(0))
        }
    };
}

impl GeneratorFactory for IteratingGeneratorFactory {
    fn bool(&mut self, _field: &str) -> Box<dyn Generator<Item = bool>> {
        Box::new(values(&[true, false]))
    }

    int_factory!(u8);
    int_factory!(u16);
    int_factory!(u32);
    int_factory!(u64);
    int_factory!(i8);
    int_factory!(i16);
    int_factory!(i32);
    int_factory!(i64);

    fn size(
        &mut self,
        _field: &str,
        list_encoding: Encoding,
        item_encoding: Encoding,
    ) -> Box<dyn Generator<Item = usize>> {
        match list_encoding {
            Encoding::Sized(size, encoding) => match *encoding {
                Encoding::Bytes => Box::new(in_bounds_and_over(size)),
                _ => unreachable!(),
            },
            Encoding::List(_) => Box::new(some_in_range(..)),
            Encoding::BoundedList(size, _) => Box::new(in_bounds_and_over(size)),
            Encoding::Bounded(size, _) => {
                // assuming elements of maximum size to be used in the list

                if let Some(element_size) = get_max_element_size(&item_encoding) {
                    let list_length = size / element_size;
                    let bounds_size = self.bounds_size.clone();
                    Box::new(in_bounds_and_over(list_length).map(move |size| {
                        *bounds_size.borrow_mut() = Some(size);
                        size
                    }))
                } else {
                    Box::new(value(1))
                }
            }
            Encoding::BoundedDynamic(size, _) => {
                // assuming elements of maximum size to be used in the list
                if let Some(element_size) = get_max_element_size(&item_encoding) {
                    let list_length = (size - 4) / element_size;
                    let bounds_size = self.bounds_size.clone();
                    Box::new(in_bounds_and_over(list_length).map(move |size| {
                        *bounds_size.borrow_mut() = Some(size);
                        size
                    }))
                } else {
                    Box::new(value(1))
                }
            }
            Encoding::Dynamic(encoding) => match *encoding {
                Encoding::List(_) => Box::new(some_in_range(..MAX_LENGTH)),
                Encoding::BoundedList(max, _) => Box::new(in_bounds_and_over(max)),
                Encoding::Dynamic(encoding) => match *encoding {
                    Encoding::List(_) => Box::new(some_in_range(..MAX_LENGTH)),
                    Encoding::BoundedList(max, _) => Box::new(in_bounds_and_over(max)),
                    _ => unreachable!(
                        "not supported in IteratingGeneratorFactory::size: {:?}",
                        encoding
                    ),
                },
                _ => unreachable!(
                    "not supported in IteratingGeneratorFactory::size: {:?}",
                    encoding
                ),
            },
            Encoding::Custom => Box::new(in_bounds_and_over(3)),
            _ => unreachable!(
                "not supported in IteratingGeneratorFactory::size: {:?}",
                list_encoding
            ),
        }
    }

    fn string(&mut self, _field: &str, encoding: Encoding) -> Box<dyn Generator<Item = String>> {
        let get_strings = |len: usize| string_of_chars("abcd1234ABCD#$%^", len);
        match encoding {
            Encoding::String => Box::new(some_in_range(..).map(get_strings)),
            Encoding::BoundedString(max_len) => {
                Box::new(in_bounds_and_over(max_len).map(get_strings))
            }
            Encoding::Bounded(max_size, encoding) => match *encoding {
                Encoding::String => Box::new(in_bounds_and_over(max_size - 4).map(get_strings)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}

struct TrivialGeneratorFactory {}

impl GeneratorFactory for TrivialGeneratorFactory {
    fn bool(&mut self, _field: &str) -> Box<dyn Generator<Item = bool>> {
        Box::new(value(false))
    }

    int_factory!(u8);
    int_factory!(u16);
    int_factory!(u32);
    int_factory!(u64);
    int_factory!(i8);
    int_factory!(i16);
    int_factory!(i32);
    int_factory!(i64);

    fn size(
        &mut self,
        _field: &str,
        list_encoding: Encoding,
        _item_encoding: Encoding,
    ) -> Box<dyn Generator<Item = usize>> {
        match list_encoding {
            Encoding::Sized(size, _) => Box::new(value(size)),
            _ => Box::new(value(1)),
        }
    }

    fn string(&mut self, _field: &str, _encoding: Encoding) -> Box<dyn Generator<Item = String>> {
        Box::new(value("s".to_string()))
    }
}

struct FillingGeneratorFactory {
    bounds_size: Rc<RefCell<Option<usize>>>,
}

impl FillingGeneratorFactory {
    fn new(bounds_size: Rc<RefCell<Option<usize>>>) -> Self {
        Self { bounds_size }
    }
}

impl GeneratorFactory for FillingGeneratorFactory {
    fn bool(&mut self, _field: &str) -> Box<dyn Generator<Item = bool>> {
        Box::new(value(false))
    }

    int_factory!(u8);
    int_factory!(u16);
    int_factory!(u32);
    int_factory!(u64);
    int_factory!(i8);
    int_factory!(i16);
    int_factory!(i32);
    int_factory!(i64);

    fn size(
        &mut self,
        _field: &str,
        list_encoding: Encoding,
        item_encoding: Encoding,
    ) -> Box<dyn Generator<Item = usize>> {
        if _field.ends_with(".fitness") {
            return Box::new(value(2));
        } else if _field.ends_with(".fitness[]") {
            return Box::new(value(8));
        }
        match list_encoding {
            Encoding::Sized(size, encoding) => match *encoding {
                Encoding::Bytes => Box::new(value(size)),
                _ => unreachable!(),
            },
            Encoding::List(_) => {
                let element_size = get_max_element_size(&item_encoding).unwrap();
                let bounds_size = self.bounds_size.clone();
                Box::new(value(()).map(move |_| {
                    bounds_size
                        .borrow()
                        .expect("Cannot determine unbound size for filling list")
                        / element_size
                }))
            }
            Encoding::BoundedList(size, _) => Box::new(value(size)),
            Encoding::Bounded(size, _) => {
                // assuming elements of maximum size to be used in the list
                let element_size = get_max_element_size(&item_encoding).unwrap();
                let list_length = size / element_size;
                Box::new(value(list_length))
            }
            Encoding::Dynamic(encoding) => match *encoding {
                Encoding::BoundedList(size, _) => Box::new(value(size)),
                Encoding::List(encoding) => {
                    let bounds_size = self.bounds_size.clone();
                    let element_size = get_max_element_size(&encoding).unwrap();
                    Box::new(value(()).map(move |_| bounds_size.borrow().unwrap() / element_size))
                }
                Encoding::Dynamic(encoding) => match *encoding {
                    Encoding::BoundedList(size, _) => Box::new(value(size)),
                    Encoding::List(encoding) => {
                        let bounds_size = self.bounds_size.clone();
                        let element_size = get_max_element_size(&encoding).unwrap();
                        Box::new(
                            value(()).map(move |_| bounds_size.borrow().unwrap() / element_size),
                        )
                    }
                    _ => unreachable!("{}: {:?}", _field, *encoding),
                },
                _ => unreachable!("{}: {:?}", _field, *encoding),
            },
            Encoding::BoundedDynamic(size, _) => {
                // assuming elements of maximum size to be used in the list
                let element_size = get_max_element_size(&item_encoding).unwrap();
                let list_length = (size - 4) / element_size;
                Box::new(value(list_length))
            }
            _ => unreachable!("{}: {:?}", _field, list_encoding),
        }
    }

    fn string(&mut self, _field: &str, encoding: Encoding) -> Box<dyn Generator<Item = String>> {
        let get_strings = |len: usize| string_of_chars("abcd1234ABCD#$%^", len);
        match encoding {
            Encoding::BoundedString(max_len) => Box::new(value(max_len).map(get_strings)),
            Encoding::Bounded(max_size, encoding) => match *encoding {
                Encoding::String => Box::new(value(max_size - 4).map(get_strings)),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }
}

struct FieldsFocus {
    fields: Vec<String>,
    focus_index: usize,
}

///
enum FieldKind {
    Focus,
    Child,
    None,
}

impl FieldsFocus {
    fn new(fields: Vec<String>, focus_index: usize) -> Self {
        Self {
            fields,
            focus_index,
        }
    }

    fn get_field_kind(focus: &str, field: &str) -> FieldKind {
        if focus == field {
            FieldKind::Focus
        } else if field.starts_with(focus) {
            FieldKind::Child
        } else {
            FieldKind::None
        }
    }

    fn field_kind(&mut self, field: &str) -> FieldKind {
        Self::get_field_kind(&self.fields[self.focus_index], field)
    }
}

struct FocusedGeneratorFactory {
    iterating: IteratingGeneratorFactory,
    filling: FillingGeneratorFactory,
    trivial: TrivialGeneratorFactory,
    fields_focus: FieldsFocus,
    bounds_size: Rc<RefCell<Option<usize>>>,
}

impl FocusedGeneratorFactory {
    fn new(fields: Vec<String>, focus_index: usize) -> Self {
        let bounds_size = Rc::new(RefCell::new(None));
        Self {
            iterating: IteratingGeneratorFactory::new(bounds_size.clone()),
            filling: FillingGeneratorFactory::new(bounds_size.clone()),
            trivial: TrivialGeneratorFactory {},
            fields_focus: FieldsFocus::new(fields, focus_index),
            bounds_size,
        }
    }

    fn generator<T: 'static + Generated>(&mut self) -> Box<dyn Generator<Item = T>> {
        let bounds_size = self.bounds_size.clone();
        Box::new(T::generator("", self).map(move |v| {
            *bounds_size.borrow_mut() = None;
            v
        }))
    }
}

macro_rules! delegate_factory {
    ($prim:ident) => {
        fn $prim(&mut self, field: &str) -> Box<dyn Generator<Item = $prim>> {
            match self.fields_focus.field_kind(field) {
                FieldKind::Focus => self.iterating.$prim(field),
                FieldKind::Child => self.filling.$prim(field),
                FieldKind::None => self.trivial.$prim(field),
            }
        }
    };
    (encoding $method:ident, $t:ty) => {
        fn $method(&mut self, field: &str, encoding: Encoding) -> Box<dyn Generator<Item = $t>> {
            match self.fields_focus.field_kind(field) {
                FieldKind::Focus => self.iterating.$method(field, encoding),
                FieldKind::Child => self.filling.$method(field, encoding),
                FieldKind::None => self.trivial.$method(field, encoding),
            }
        }
    };
    (encoding2 $method:ident, $t:ty) => {
        fn $method(
            &mut self,
            field: &str,
            encoding: Encoding,
            element_encoding: Encoding,
        ) -> Box<dyn Generator<Item = $t>> {
            match self.fields_focus.field_kind(field) {
                FieldKind::Focus => self.iterating.$method(field, encoding, element_encoding),
                FieldKind::Child => self.filling.$method(field, encoding, element_encoding),
                FieldKind::None => self.trivial.$method(field, encoding, element_encoding),
            }
        }
    };
}

impl GeneratorFactory for FocusedGeneratorFactory {
    delegate_factory!(bool);

    delegate_factory!(u8);
    delegate_factory!(u16);
    delegate_factory!(u32);
    delegate_factory!(u64);
    delegate_factory!(i8);
    delegate_factory!(i16);
    delegate_factory!(i32);
    delegate_factory!(i64);

    delegate_factory!(encoding2 size, usize);
    delegate_factory!(encoding string, String);
}

trait TestFeedback {
    fn error(&mut self, error: BinError);
}

struct EmptyFeedback;

impl TestFeedback for EmptyFeedback {
    fn error(&mut self, _error: BinError) {}
}

struct LimitCoverageFeedback {
    pub covered_fields: HashSet<String>,
}

impl LimitCoverageFeedback {
    fn new() -> Self {
        Self {
            covered_fields: HashSet::new(),
        }
    }
}

impl TestFeedback for LimitCoverageFeedback {
    fn error(&mut self, error: BinError) {
        let mut errors = error.iter();
        if let Some(err) = errors.next() {
            if let Some(BinErrorKind::FieldError(field_name)) = errors.next() {
                match err {
                    BinErrorKind::SizeError(_exp, _act) => {
                        self.covered_fields.insert(field_name.to_string());
                    }
                    BinErrorKind::CustomError(_err) => {
                        self.covered_fields.insert(field_name.to_string());
                    }
                    _ => (),
                }
            }
        }
    }
}

fn test_message_with_factory<
    M: 'static + HasEncoding + Serialize + BinWriter + Generated,
    F: TestFeedback,
>(
    factory: &mut FocusedGeneratorFactory,
    feedback: &mut F,
) {
    for msg in factory.generator::<M>().iter() {
        if let Err(e) = diff_encodings_with_error(&msg) {
            feedback.error(e);
        }
    }
}

fn test_peer_message_with_factory<
    M: 'static + HasEncoding + Serialize + BinWriter + Generated + Into<PeerMessage>,
    F: TestFeedback,
>(
    factory: &mut FocusedGeneratorFactory,
    feedback: &mut F,
) {
    for msg in factory.generator::<M>().iter() {
        if let Err(e) = diff_encodings_with_error(&msg) {
            feedback.error(e);
        }
        let peer_message: PeerMessage = msg.into();
        let peer_message_response: PeerMessageResponse = peer_message.into();
        if let Err(e) = diff_encodings_with_error(&peer_message_response) {
            feedback.error(e);
        }
    }
}

fn test_message<M: 'static + HasEncoding + Serialize + BinWriter + Generated>() {
    test_message_with_feedback::<M, _>(&mut EmptyFeedback)
}

fn test_peer_message<
    M: 'static + HasEncoding + Serialize + BinWriter + Generated + Into<PeerMessage>,
>() {
    test_peer_message_with_feedback::<M, _>(&mut EmptyFeedback)
}

fn test_message_with_feedback<
    M: 'static + HasEncoding + Serialize + BinWriter + Generated,
    F: TestFeedback,
>(
    feedback: &mut F,
) {
    let fields = get_all_fields::<M>();
    for i in 0..fields.len() {
        let mut factory = FocusedGeneratorFactory::new(fields.clone(), i);
        test_message_with_factory::<M, _>(&mut factory, feedback);
    }
}

fn test_peer_message_with_feedback<
    M: 'static + HasEncoding + Serialize + BinWriter + Generated + Into<PeerMessage>,
    F: TestFeedback,
>(
    feedback: &mut F,
) {
    let fields = get_all_fields::<M>();
    for i in 0..fields.len() {
        let mut factory = FocusedGeneratorFactory::new(fields.clone(), i);
        test_peer_message_with_factory::<M, _>(&mut factory, feedback);
    }
}

fn needs_boundary_checking(encoding: &Encoding, fields: &mut HashSet<String>) -> bool {
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
        | Encoding::RangedFloat => false,

        Encoding::Z | Encoding::Mutez => true,

        Encoding::Enum => false,
        Encoding::String => true,
        Encoding::BoundedString(_) => true,
        Encoding::Bytes => true,
        Encoding::List(encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::BoundedList(_, encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::Option(encoding) => needs_boundary_checking(encoding, fields),
        Encoding::OptionalField(encoding) => needs_boundary_checking(encoding, fields),
        Encoding::Dynamic(encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::BoundedDynamic(_, encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::Sized(_, encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::Bounded(_, encoding) => {
            needs_boundary_checking(encoding, fields);
            true
        }
        Encoding::Tup(encodings) => encodings.into_iter().fold(false, |b, encoding| {
            needs_boundary_checking(encoding, fields) || b
        }),

        Encoding::Obj(ty, flds) => flds.iter().fold(false, |b, field| {
            if needs_boundary_checking(field.get_encoding(), fields) {
                fields.insert(format!("{}::{}", ty, field.get_name()));
                true
            } else {
                b
            }
        }),
        Encoding::Tags(_, tags) => tags.tags().fold(false, |b, tag| {
            if needs_boundary_checking(tag.get_encoding(), fields) {
                true
            } else {
                b
            }
        }),

        Encoding::Hash(_) => false,
        Encoding::Timestamp => false,
        Encoding::Custom => true,
        _ => unimplemented!(),
    }
}

fn get_all_fields<M: HasEncoding>() -> Vec<String> {
    let mut res = Vec::new();
    get_focus_fields(M::encoding(), "", &mut res);
    res.sort();
    res
}

fn get_focus_fields(encoding: &Encoding, prefix: &str, fields: &mut Vec<String>) -> bool {
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
        | Encoding::RangedFloat => false,

        Encoding::Z | Encoding::Mutez => true,

        Encoding::Enum => false,
        Encoding::String => true,
        Encoding::BoundedString(_) => true,
        Encoding::Bytes => true,
        Encoding::List(encoding) => {
            let prefix = prefix.to_string() + "[]";
            fields.push(prefix.clone());
            get_focus_fields(encoding, &prefix, fields);
            true
        }
        Encoding::BoundedList(_, encoding) => {
            get_focus_fields(encoding, prefix, fields);
            true
        }
        Encoding::Option(encoding) => {
            let prefix = prefix.to_string() + "?";
            fields.push(prefix.clone());
            get_focus_fields(encoding, &prefix, fields)
        }
        Encoding::OptionalField(encoding) => {
            let prefix = prefix.to_string() + "?";
            fields.push(prefix.clone());
            get_focus_fields(encoding, &prefix, fields)
        }
        Encoding::Dynamic(encoding) => {
            get_focus_fields(encoding, prefix, fields);
            true
        }
        Encoding::BoundedDynamic(_, encoding) => {
            get_focus_fields(encoding, prefix, fields);
            true
        }
        Encoding::Sized(_, encoding) => {
            get_focus_fields(encoding, prefix, fields);
            true
        }
        Encoding::Bounded(_, encoding) => {
            get_focus_fields(encoding, prefix, fields);
            true
        }
        Encoding::Tup(encodings) => encodings.into_iter().fold(false, |b, encoding| {
            let prefix = prefix.to_string() + "...";
            fields.push(prefix.clone());
            get_focus_fields(encoding, &prefix, fields) || b
        }),

        Encoding::Obj(_, flds) => flds.iter().fold(false, |b, field| {
            let prefix = prefix.to_string() + "." + field.get_name();
            fields.push(prefix.clone());
            if get_focus_fields(field.get_encoding(), &prefix, fields) {
                true
            } else {
                b
            }
        }),
        Encoding::Tags(_, tags) => tags.tags().fold(false, |b, tag| {
            let prefix = prefix.to_string() + "." + tag.get_variant();
            fields.push(prefix.clone());
            if get_focus_fields(tag.get_encoding(), &prefix, fields) {
                true
            } else {
                b
            }
        }),

        Encoding::Hash(_) => false,
        Encoding::Timestamp => false,
        Encoding::Custom => true,
        _ => unimplemented!(),
    }
}

#[test]
fn connection() {
    test_message::<ConnectionMessage>()
}

#[test]
fn metadata() {
    test_message::<MetadataMessage>()
}

#[test]
fn ack() {
    test_message::<AckMessage>()
}

#[test]
fn swap() {
    test_message::<SwapMessage>()
}

#[test]
fn advertise() {
    test_peer_message::<AdvertiseMessage>()
}

#[test]
fn current_branch() {
    test_peer_message::<GetCurrentBranchMessage>();
    test_peer_message::<CurrentBranchMessage>();
}

#[test]
fn current_head() {
    test_peer_message::<GetCurrentHeadMessage>();
    test_peer_message::<CurrentHeadMessage>();
}

#[test]
fn block_headers() {
    test_peer_message::<GetBlockHeadersMessage>();
    test_peer_message::<BlockHeaderMessage>();
}

#[test]
fn operations() {
    test_peer_message::<GetOperationsMessage>();
    test_peer_message::<OperationMessage>();
}

#[test]
fn protocols() {
    test_peer_message::<GetProtocolsMessage>();
    test_peer_message::<ProtocolMessage>();
}

#[test]
fn operations_for_blocks() {
    test_peer_message::<GetOperationsForBlocksMessage>();
    test_peer_message::<OperationsForBlocksMessage>();
}

#[test]
#[ignore = "Long-running test that reports per-field limits violation coverage"]
fn limits_coverage() {
    let mut feedback = LimitCoverageFeedback::new();
    test_message_with_feedback::<ConnectionMessage, _>(&mut feedback);
    test_message_with_feedback::<MetadataMessage, _>(&mut feedback);
    test_message_with_feedback::<AckMessage, _>(&mut feedback);
    test_message_with_feedback::<SwapMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<AdvertiseMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetCurrentBranchMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<CurrentBranchMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetCurrentHeadMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<CurrentHeadMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetBlockHeadersMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<BlockHeaderMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetOperationsMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<OperationMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetProtocolsMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<ProtocolMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<GetOperationsForBlocksMessage, _>(&mut feedback);
    test_peer_message_with_feedback::<OperationsForBlocksMessage, _>(&mut feedback);

    let mut all_fields = HashSet::new();
    needs_boundary_checking(ConnectionMessage::encoding(), &mut all_fields);
    needs_boundary_checking(MetadataMessage::encoding(), &mut all_fields);
    needs_boundary_checking(AckMessage::encoding(), &mut all_fields);
    needs_boundary_checking(PeerMessageResponse::encoding(), &mut all_fields);

    let mut all_fields = all_fields.into_iter().collect::<Vec<_>>();
    all_fields.sort();
    for field in all_fields {
        println!(
            "{:60}   {}",
            field,
            feedback.covered_fields.contains(&field)
        );
    }
}
