// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use once_cell::sync::Lazy as SyncLazy;

use parse_display::{Display, FromStr};
use proc_macro2::Span;

#[derive(Debug)]
pub struct DataWithEncoding<'a> {
    pub name: &'a syn::Ident,
    pub encoding: Encoding<'a>,
}

#[derive(Debug)]
pub struct StructEncoding<'a> {
    pub name: &'a syn::Ident,
    pub fields: Vec<FieldEncoding<'a>>,
}

#[derive(Debug)]
pub enum FieldKind<'a> {
    Encoded(Box<EncodedField<'a>>),
    Hash,
    Skip,
}

#[derive(Debug)]
pub struct FieldEncoding<'a> {
    pub name: &'a syn::Ident,
    pub kind: FieldKind<'a>,
}

impl<'a> FieldEncoding<'a> {
    pub fn encoding(&'a self) -> Option<&Encoding<'a>> {
        match &self.kind {
            FieldKind::Encoded(encoded_field) => Some(&encoded_field.encoding),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct EncodedField<'a> {
    pub encoding: Encoding<'a>,
    pub reserve: Option<syn::Expr>,
}

#[derive(Debug)]
pub struct EnumEncoding<'a> {
    pub name: &'a syn::Ident,
    pub tag_type: syn::Ident,
    pub ignore_unknown: bool,
    pub tags: Vec<Tag<'a>>,
}

#[derive(Debug)]
pub struct Tag<'a> {
    pub id: syn::LitInt,
    pub name: &'a syn::Ident,
    pub encoding: Encoding<'a>,
}

#[derive(Debug)]
pub enum Encoding<'a> {
    Unit,
    Primitive(PrimitiveEncoding, Span),
    Bytes(Span),
    Path(&'a syn::Path),
    Zarith(Span),
    MuTez(Span),

    String(Option<syn::Expr>, Span),

    Struct(StructEncoding<'a>),
    Enum(EnumEncoding<'a>),

    OptionField(Box<Encoding<'a>>, Span),
    List(Option<syn::Expr>, Box<Encoding<'a>>, Span),

    Sized(syn::Expr, Box<Encoding<'a>>, Span),
    Bounded(syn::Expr, Box<Encoding<'a>>, Span),
    ShortDynamic(Box<Encoding<'a>>, Span),
    Dynamic(Option<syn::Expr>, Box<Encoding<'a>>, Span),
}

#[derive(Clone, Copy, Debug, PartialEq, Display, FromStr)]
pub enum PrimitiveEncoding {
    Int8,
    Uint8,
    Int16,
    Uint16,
    Int31,
    Int32,
    Uint32,
    Int64,
    Float,
    Bool,
    Timestamp,
}

impl PrimitiveEncoding {
    pub fn make_ident(&self, span: Span) -> syn::Ident {
        syn::Ident::new(&self.to_string(), span)
    }
}

impl syn::parse::Parse for PrimitiveEncoding {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        name.to_string()
            .parse()
            .map_err(|_| input.error("Unrecognized primitive encoding"))
    }
}

pub fn get_primitive_number_mapping(kind: PrimitiveEncoding) -> Option<&'static str> {
    static PRIMITIVE_NUMBERS_MAPPING: SyncLazy<Vec<(PrimitiveEncoding, &'static str)>> =
        SyncLazy::new(|| {
            use crate::encoding::PrimitiveEncoding::*;
            vec![
                (Bool, "bool"),
                (Int8, "i8"),
                (Uint8, "u8"),
                (Int16, "i16"),
                (Uint16, "u16"),
                (Int31, "i32"),
                (Int32, "i32"),
                (Uint32, "u32"),
                (Int64, "i64"),
                (Float, "f64"),
                (Timestamp, "i64"),
            ]
        });
    PRIMITIVE_NUMBERS_MAPPING
        .iter()
        .find_map(|(k, s)| if kind == *k { Some(*s) } else { None })
}
