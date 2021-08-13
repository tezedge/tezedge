// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

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
    Encoded(Encoding<'a>),
    Hash,
    Skip,
}

#[derive(Debug)]
pub struct FieldEncoding<'a> {
    pub name: &'a syn::Ident,
    pub kind: FieldKind<'a>,
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
