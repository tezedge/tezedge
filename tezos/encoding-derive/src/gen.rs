// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::encoding::*;
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};

pub fn generate_for_data(data: &DataWithEncoding) -> TokenStream {
    let name = data.name;
    let generator = generator_for_data(&data.encoding, quote!(prefix.to_string()));
    let generator = quote_spanned! {
        data.name.span()=>
        #[allow(unused_parens)]
        #[allow(unused_variables)]
        #[allow(clippy::unnecessary_cast)]
        impl tezos_encoding::generator::Generated for #name {
            fn generator<F: tezos_encoding::generator::GeneratorFactory>(prefix: &str, f: &mut F) -> Box<dyn tezos_encoding::generator::Generator<Item = Self>> {
                Box::new(#generator)
            }
        }
    };
    generator
}

fn generator_for_data(encoding: &Encoding, prefix: TokenStream) -> TokenStream {
    match encoding {
        Encoding::Struct(encoding) => generate_struct(encoding, prefix),
        Encoding::Enum(encoding) => generate_enum(encoding, prefix),
        Encoding::Bounded(_size, encoding, _span) => generator_for_data(encoding, prefix),
        Encoding::Dynamic(_size, encoding, _span) => generator_for_data(encoding, prefix),
        _ => unreachable!("not supported by generator_for_data: {:?}", encoding),
    }
}

enum GeneratorKind<'a> {
    /// Factory method for primitive type generators.
    FactoryPrimitive(&'static str, Span),
    /// Factory method for type generator with encoding.
    FactoryWithEncoding(&'static str, Span),
    /// Factory method for type generator with two encodings.
    FactoryWithEncoding2(&'static str, &'a Encoding<'a>, Span),
    /// Type providing its own generator.
    GeneratedType(&'a syn::Path),
}

impl<'a> GeneratorKind<'a> {
    fn generate(self, encoding: &Encoding, field_name: TokenStream) -> TokenStream {
        match self {
            GeneratorKind::FactoryPrimitive(prim, span) => {
                let method = format_ident!("{}", prim);
                quote_spanned!(span=> f.#method(&(#field_name)))
            }
            GeneratorKind::FactoryWithEncoding(ty, span) => {
                let method = format_ident!("{}", ty);
                let encoding = super::enc::generate_encoding(encoding);
                quote_spanned!(span=> f.#method(&(#field_name), #encoding))
            }
            GeneratorKind::FactoryWithEncoding2(ty, encoding2, span) => {
                let method = format_ident!("{}", ty);
                let encoding = super::enc::generate_encoding(encoding);
                let encoding2 = super::enc::generate_encoding(encoding2);
                quote_spanned!(span=> f.#method(&(#field_name), #encoding, #encoding2))
            }
            GeneratorKind::GeneratedType(ty) => {
                quote!(#ty::generator(&(#field_name), f))
            }
        }
    }
}

fn decompose_encoding<'a>(
    encoding: &'a Encoding<'a>,
    prefix: TokenStream,
) -> (
    GeneratorKind<'a>,
    Option<(&'a Encoding<'a>, &'static str, TokenStream)>,
) {
    match encoding {
        Encoding::Primitive(primitive, span) => (
            GeneratorKind::FactoryPrimitive(
                get_primitive_number_mapping(*primitive).unwrap(),
                *span,
            ),
            None,
        ),
        Encoding::Bytes(span) => (GeneratorKind::FactoryPrimitive("u8", *span), None),
        Encoding::String(_, span) => (GeneratorKind::FactoryWithEncoding("string", *span), None),
        Encoding::Path(path) => (GeneratorKind::GeneratedType(path), None),
        Encoding::List(_, encoding, span) => (
            GeneratorKind::FactoryWithEncoding2("size", encoding, *span),
            Some((encoding, "vec_of_items", quote!(#prefix + "[]"))),
        ),
        Encoding::Sized(_, encoding, span) => (
            GeneratorKind::FactoryWithEncoding2("size", encoding, *span),
            Some((encoding, "vec_of_items", quote!(#prefix + "[]"))),
        ),
        Encoding::OptionField(encoding, span) => (
            GeneratorKind::FactoryPrimitive("bool", *span),
            Some((encoding, "option", quote!(#prefix + "?"))),
        ),
        Encoding::Bounded(_, encoding, _) => decompose_encoding(encoding, prefix),
        Encoding::Dynamic(_, encoding, _) => decompose_encoding(encoding, prefix),
        _ => unreachable!("not supported by decompose_encoding: {:?}", encoding),
    }
}

fn generator_for_encoding(encoding: &Encoding, prefix: TokenStream) -> TokenStream {
    let (method_kind, decomposed) = decompose_encoding(encoding, prefix.clone());
    let generator = method_kind.generate(encoding, prefix);
    if let Some((inner_encoding, compose_func, prefix)) = decomposed {
        let compose_func = format_ident!("{}", compose_func);
        let inner_generator = generator_for_encoding(inner_encoding, prefix);
        quote!(tezos_encoding::generator::#compose_func(#inner_generator, #generator))
    } else {
        generator
    }
}

fn generate_struct(encoding: &StructEncoding, prefix: TokenStream) -> TokenStream {
    if encoding.fields.len() == 1 {
        generate_struct_single_field(encoding, prefix)
    } else {
        generate_struct_multi_fields(encoding, prefix)
    }
}

fn generate_struct_single_field(encoding: &StructEncoding, prefix: TokenStream) -> TokenStream {
    let field = encoding.fields.first().unwrap();
    let field_encoding = generate_struct_field(field, prefix);
    let field_name = field.name;
    quote_spanned! {
        encoding.name.span()=>
            tezos_encoding::generator::map(
                #field_encoding,
                |#field_name| Self { #field_name }
            )
    }
}

fn generate_struct_multi_fields(encoding: &StructEncoding, prefix: TokenStream) -> TokenStream {
    let field_encoding = encoding
        .fields
        .iter()
        .map(|f| generate_struct_field(f, prefix.clone()))
        .filter(|ts| !ts.is_empty());
    let field_name1 = encoding.fields.iter().filter_map(|f| {
        if f.encoding().is_some() {
            Some(f.name)
        } else {
            None
        }
    });
    let field_name2 = encoding.fields.iter().map(|f| {
        let name = f.name;
        if f.encoding().is_some() {
            quote!(#name)
        } else {
            quote!(#name: Default::default())
        }
    });
    quote_spanned! {
        encoding.name.span()=>
            tezos_encoding::generator::map(
                tezos_encoding::generator::tuple((
                    #(#field_encoding),*
                )),
                |(#(#field_name1),*)| {
                    Self {
                        #(#field_name2,)*
                    }
                }
            )
    }
}

fn generate_struct_field(field: &FieldEncoding, prefix: TokenStream) -> TokenStream {
    let encoding = if let FieldKind::Encoded(encoding) = &field.kind {
        encoding
    } else {
        return TokenStream::default();
    };
    let field_name_str = ".".to_string() + &field.name.to_string();
    generator_for_encoding(encoding, quote!(#prefix + #field_name_str))
}

fn generate_enum(encoding: &EnumEncoding, prefix: TokenStream) -> TokenStream {
    let mut tags = encoding.tags.iter();
    let first = tags.next().unwrap();
    let first_generator = generate_variant(encoding.name, first, prefix.clone());
    let tags_generator = tags.fold(first_generator, |ts, tag| {
        let tag_generator = generate_variant(encoding.name, tag, prefix.clone());
        quote_spanned! {tag.name.span()=>
            tezos_encoding::generator::Generator::and(#ts, #tag_generator)
        }
    });
    quote!(#tags_generator)
}

fn generate_variant(enum_name: &syn::Ident, tag: &Tag, prefix: TokenStream) -> TokenStream {
    let tag_name = tag.name;
    let tag_name_str = ".".to_string() + &tag.name.to_string();
    let full_name = quote!(#enum_name::#tag_name);
    match tag.encoding {
        Encoding::Unit => {
            quote_spanned!(tag_name.span()=> tezos_encoding::generator::value(#full_name))
        }
        _ => {
            let generator = generator_for_encoding(&tag.encoding, quote!(#prefix + #tag_name_str));
            quote_spanned!(tag_name.span()=> tezos_encoding::generator::map(#generator, #full_name))
        }
    }
}
