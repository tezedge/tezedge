// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::encoding::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub fn generate_encoding_for_data(data: &DataWithEncoding) -> TokenStream {
    let name = data.name;
    let encoding = generate_encoding(&data.encoding);
    quote_spanned! {data.name.span()=>
        impl tezos_encoding::encoding::HasEncoding for #name {
            fn encoding() -> tezos_encoding::encoding::Encoding {
                #encoding
            }
        }
    }
}

pub(crate) fn generate_encoding(encoding: &Encoding) -> TokenStream {
    match encoding {
        Encoding::Unit => quote!(tezos_encoding::encoding::Encoding::Unit),
        Encoding::Primitive(primitive, span) => generage_primitive_encoding(*primitive, *span),
        Encoding::Bytes(span) => quote_spanned!(*span=> tezos_encoding::encoding::Encoding::Bytes),
        Encoding::Path(path) => {
            quote_spanned!(path.span()=> <#path as tezos_encoding::encoding::HasEncoding>::encoding().clone())
        }
        Encoding::String(size, span) => generate_string_encoding(size, *span),
        Encoding::Struct(encoding) => generate_struct_encoding(encoding),
        Encoding::Enum(encoding) => generate_enum_encoding(encoding),
        Encoding::OptionField(encoding, span) => generate_optional_field_encoding(encoding, *span),
        Encoding::List(size, encoding, span) => generate_list_encoding(size, encoding, *span),
        Encoding::Sized(size, encoding, span) => generate_sized_encoding(size, encoding, *span),
        Encoding::Bounded(size, encoding, span) => generate_bounded_encoding(size, encoding, *span),
        Encoding::ShortDynamic(encoding, span) => generate_short_dynamic_encoding(encoding, *span),
        Encoding::Dynamic(size, encoding, span) => generate_dynamic_encoding(size, encoding, *span),
        Encoding::Zarith(span) => quote_spanned!(*span=> tezos_encoding::encoding::Encoding::Z),
        Encoding::MuTez(span) => quote_spanned!(*span=> tezos_encoding::encoding::Encoding::Mutez),
    }
}

fn generage_primitive_encoding(kind: PrimitiveEncoding, span: Span) -> TokenStream {
    let ident = kind.make_ident(span);
    quote_spanned!(ident.span()=> tezos_encoding::encoding::Encoding::#ident)
}

fn generate_struct_encoding(encoding: &StructEncoding) -> TokenStream {
    let name_str = encoding.name.to_string();
    let fields_encoding = encoding.fields.iter().filter_map(generate_field_encoding);
    quote_spanned! { encoding.name.span()=>
        tezos_encoding::encoding::Encoding::Obj(#name_str, vec![
            #(#fields_encoding),*
        ])
    }
}

fn generate_field_encoding(field: &FieldEncoding) -> Option<TokenStream> {
    if let FieldKind::Encoded(encoding) = &field.kind {
        let name = field.name.to_string();
        let encoding = generate_encoding(&encoding.encoding);
        Some(
            quote_spanned!(field.name.span()=> tezos_encoding::encoding::Field::new(#name, #encoding)),
        )
    } else {
        None
    }
}

fn generate_enum_encoding(encoding: &EnumEncoding) -> TokenStream {
    let tag_type = &encoding.tag_type;
    let tags_encoding = encoding.tags.iter().map(generate_tag_encoding);
    quote_spanned! { tag_type.span()=>
        tezos_encoding::encoding::Encoding::Tags(
            std::mem::size_of::<#tag_type>(),
            tezos_encoding::encoding::TagMap::new(vec![
                #(#tags_encoding),*
            ])
        )
    }
}

fn generate_tag_encoding(tag: &Tag) -> TokenStream {
    let id = &tag.id;
    let name = tag.name.to_string();
    let encoding = generate_encoding(&tag.encoding);
    quote_spanned!(tag.name.span()=> tezos_encoding::encoding::Tag::new(#id, #name, #encoding))
}

fn generate_string_encoding(size: &Option<syn::Expr>, span: Span) -> TokenStream {
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::encoding::Encoding::String),
        |size| quote_spanned!(span=> tezos_encoding::encoding::Encoding::BoundedString(#size)),
    )
}

fn generate_list_encoding<'a>(
    size: &Option<syn::Expr>,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let encoding = generate_encoding(encoding);
    size.as_ref().map_or_else(|| quote_spanned!(span=> tezos_encoding::encoding::Encoding::List(Box::new(#encoding))), |size| quote_spanned!(span=> tezos_encoding::encoding::Encoding::BoundedList(#size, Box::new(#encoding))))
}

fn generate_optional_field_encoding(encoding: &Encoding, span: Span) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote_spanned!(span=> tezos_encoding::encoding::Encoding::OptionalField(Box::new(#encoding)))
}

fn generate_sized_encoding<'a>(
    size: &syn::Expr,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote_spanned!(span=> tezos_encoding::encoding::Encoding::Sized(#size, Box::new(#encoding)))
}

fn generate_bounded_encoding<'a>(
    size: &syn::Expr,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote_spanned!(span=> tezos_encoding::encoding::Encoding::Bounded(#size, Box::new(#encoding)))
}

fn generate_short_dynamic_encoding<'a>(encoding: &Encoding<'a>, span: Span) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote_spanned!(span=> tezos_encoding::encoding::Encoding::ShortDynamic(Box::new(#encoding)))
}

fn generate_dynamic_encoding<'a>(
    size: &Option<syn::Expr>,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let encoding = generate_encoding(encoding);
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::encoding::Encoding::Dynamic(Box::new(#encoding))),
        |size| quote_spanned!(span=> tezos_encoding::encoding::Encoding::BoundedDynamic(#size, Box::new(#encoding))))
}
