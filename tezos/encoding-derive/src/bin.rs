// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::encoding::*;
use proc_macro2::{Span, TokenStream};
use quote::quote_spanned;
use syn::spanned::Spanned;

pub fn generate_bin_write_for_data(data: &DataWithEncoding) -> TokenStream {
    let name = data.name;
    let bin_write = generate_bin_write(&data.encoding);
    quote_spanned! {
        data.name.span()=>
        #[allow(unused_parens)]
        #[allow(clippy::unnecessary_cast)]
        impl tezos_encoding::enc::BinWriter for #name {
            fn bin_write(&self, out: &mut Vec<u8>) -> tezos_encoding::enc::BinResult {
                #bin_write(self, out)
            }
        }
    }
}

fn generate_bin_write(encoding: &Encoding) -> TokenStream {
    match encoding {
        Encoding::Unit => unreachable!(),
        Encoding::Primitive(primitive, span) => generage_primitive_bin_write(*primitive, *span),
        Encoding::Bytes(span) => generate_bytes_bin_write(*span),
        Encoding::Path(path) => {
            quote_spanned!(path.span()=> <#path as tezos_encoding::enc::BinWriter>::bin_write)
        }
        Encoding::Struct(encoding) => generate_struct_bin_write(encoding),
        Encoding::Enum(encoding) => generate_enum_bin_write(encoding),
        Encoding::String(size, span) => generate_string_bin_write(size, *span),
        Encoding::OptionField(encoding, span) => generate_optional_field_bin_write(encoding, *span),
        Encoding::List(size, encoding, span) => generate_list_bin_write(size, encoding, *span),
        Encoding::Sized(size, encoding, span) => generate_sized_bin_write(size, encoding, *span),
        Encoding::Bounded(size, encoding, span) => {
            generate_bounded_bin_write(size, encoding, *span)
        }
        Encoding::Dynamic(size, encoding, span) => {
            generate_dynamic_bin_write(size, encoding, *span)
        }
        Encoding::Zarith(span) => quote_spanned!(*span=> tezos_encoding::enc::zarith),
        Encoding::MuTez(span) => quote_spanned!(*span=> tezos_encoding::enc::mutez),
    }
}

fn generate_bytes_bin_write(span: Span) -> TokenStream {
    quote_spanned!(span=> tezos_encoding::enc::bytes)
}

fn generage_primitive_bin_write(kind: PrimitiveEncoding, span: Span) -> TokenStream {
    match kind {
        PrimitiveEncoding::Int8
        | PrimitiveEncoding::Uint8
        | PrimitiveEncoding::Int16
        | PrimitiveEncoding::Uint16
        | PrimitiveEncoding::Int31
        | PrimitiveEncoding::Int32
        | PrimitiveEncoding::Uint32
        | PrimitiveEncoding::Int64
        | PrimitiveEncoding::Float
        | PrimitiveEncoding::Timestamp => {
            generate_number_bin_write(get_primitive_number_mapping(kind).unwrap(), span)
        }
        PrimitiveEncoding::Bool => quote_spanned!(span=> tezos_encoding::enc::boolean),
    }
}

fn generate_number_bin_write(num: &str, span: Span) -> TokenStream {
    let ty = syn::Ident::new(num, span);
    quote_spanned!(span=> tezos_encoding::enc::#ty)
}

fn generate_struct_bin_write(encoding: &StructEncoding) -> TokenStream {
    let fields_with_encoding = encoding.fields.iter().filter(|f| f.encoding().is_some());
    let field = fields_with_encoding.clone().map(|f| f.name);
    let field_name = fields_with_encoding
        .clone()
        .map(|f| format!("{}::{}", encoding.name, f.name.to_string()));
    let field_bin_write =
        fields_with_encoding.map(|f| generate_struct_field_bin_write(&f.encoding().unwrap()));
    quote_spanned! {
        encoding.name.span()=>
            (|data: &Self, out: &mut Vec<u8>| {
                #(
                    tezos_encoding::enc::field(#field_name, #field_bin_write)(&data.#field, out)?;
                )*
                Ok(())
            })
    }
}

fn generate_struct_field_bin_write(encoding: &Encoding) -> TokenStream {
    generate_bin_write(encoding)
}

fn generate_enum_bin_write(encoding: &EnumEncoding) -> TokenStream {
    let tag_type = &encoding.tag_type;
    let tag_serialize = quote_spanned!(encoding.tag_type.span()=> tezos_encoding::enc::#tag_type);
    let tags_bin_write = encoding
        .tags
        .iter()
        .map(|tag| generate_tag_bin_write(tag, encoding.name, &tag_serialize));
    quote_spanned! {
        tag_type.span()=>
            (|data: &Self, out| {
                match data {
                    #(#tags_bin_write),*
                }
            })
    }
}

fn generate_tag_bin_write<'a>(
    tag: &Tag<'a>,
    enum_name: &syn::Ident,
    tag_encoding: &TokenStream,
) -> TokenStream {
    let tag_name = tag.name;
    let tag_id = &tag.id;
    let name = format!("{}::{}", enum_name, tag_name);
    match &tag.encoding {
        Encoding::Unit => {
            quote_spanned!(tag_name.span()=>
                           #enum_name::#tag_name => tezos_encoding::enc::variant(#name, #tag_encoding)(&#tag_id, out)
            )
        }
        encoding => {
            let bin_write = generate_bin_write(&encoding);
            quote_spanned!(tag_name.span()=>
                           #enum_name::#tag_name(inner) => tezos_encoding::enc::variant_with_field(#name, #tag_encoding, #bin_write)(&#tag_id, &inner, out)
            )
        }
    }
}

fn generate_string_bin_write(size: &Option<syn::Expr>, span: Span) -> TokenStream {
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::enc::string),
        |size| quote_spanned!(span=> tezos_encoding::enc::bounded_string(#size)),
    )
}

fn generate_optional_field_bin_write(encoding: &Encoding, span: Span) -> TokenStream {
    let bin_write = generate_bin_write(encoding);
    quote_spanned!(span=> tezos_encoding::enc::optional_field(#bin_write))
}

fn generate_list_bin_write(
    size: &Option<syn::Expr>,
    encoding: &Encoding,
    span: Span,
) -> TokenStream {
    let bin_write = generate_bin_write(encoding);
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::enc::list(#bin_write)),
        |size| quote_spanned!(span=> tezos_encoding::enc::bounded_list(#size, #bin_write)),
    )
}

fn generate_sized_bin_write(size: &syn::Expr, encoding: &Encoding, span: Span) -> TokenStream {
    let bin_write = generate_bin_write(encoding);
    quote_spanned!(span=> tezos_encoding::enc::sized(#size, #bin_write))
}

fn generate_bounded_bin_write(size: &syn::Expr, encoding: &Encoding, span: Span) -> TokenStream {
    let bin_write = generate_bin_write(encoding);
    quote_spanned!(span=> tezos_encoding::enc::bounded(#size, #bin_write))
}

fn generate_dynamic_bin_write(
    size: &Option<syn::Expr>,
    encoding: &Encoding,
    span: Span,
) -> TokenStream {
    let bin_write = generate_bin_write(encoding);
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::enc::dynamic(#bin_write)),
        |size| quote_spanned!(span=> tezos_encoding::enc::bounded_dynamic(#size, #bin_write)),
    )
}
