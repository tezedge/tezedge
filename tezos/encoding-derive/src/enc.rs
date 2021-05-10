use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use crate::encoding::*;

pub fn generate_encoding_for_data<'a>(data: &DataWithEncoding<'a>) -> TokenStream {
    let name = data.name;
    let name_str = name.to_string();
    let encoding_static_name = syn::Ident::new(
        &format!("__TEZOS_ENCODING_{}", name_str.to_uppercase()),
        name.span(),
    );
    let encoding = generate_encoding(&data.encoding);
    quote_spanned! {data.name.span()=>
        lazy_static::lazy_static! {
            static ref #encoding_static_name: tezos_encoding::encoding::Encoding = #encoding;
        }

        impl tezos_encoding::encoding::HasEncoding for #name {
            fn encoding() -> &'static tezos_encoding::encoding::Encoding {
                &#encoding_static_name
            }
        }
    }
}

fn generate_encoding<'a>(encoding: &Encoding<'a>) -> TokenStream {
    match encoding {
        Encoding::Unit => quote!(tezos_encoding::encoding::Encoding::Unit),
        Encoding::Primitive(primitive) => generage_primitive_encoding(primitive),
        Encoding::Bytes => quote!(tezos_encoding::encoding::Encoding::Bytes),
        Encoding::Path(path) => quote!(#path::encoding().clone()),
        Encoding::Struct(encoding) => generate_struct_encoding(encoding),
        Encoding::Enum(encoding) => generate_enum_encoding(encoding),
        Encoding::String(size) => generate_string_encoding(size),
        Encoding::List(size, encoding) => generate_list_encoding(size, encoding),
        Encoding::Sized(size, encoding) => generate_sized_encoding(size, encoding),
        Encoding::Bounded(size, encoding) => generate_bounded_encoding(size, encoding),
        Encoding::Dynamic(size, encoding) => generate_dynamic_encoding(size, encoding),
    }
}

fn generage_primitive_encoding(ident: &syn::Ident) -> TokenStream {
    let mapped = syn::Ident::new(crate::symbol::PRIMITIVE_MAPPING.get(ident.to_string().as_str()).unwrap(), ident.span());
    quote_spanned!(ident.span()=> tezos_encoding::encoding::Encoding::#mapped)
}

fn generate_struct_encoding(encoding: &StructEncoding) -> TokenStream {
    let name_str = encoding.name.to_string();
    let fields_encoding = encoding.fields.iter().map(generate_field_encoding);
    quote_spanned! { encoding.name.span()=>
        tezos_encoding::encoding::Encoding::Obj(#name_str, vec![
            #(#fields_encoding),*
        ])
    }
}

fn generate_field_encoding<'a>(field: &FieldEncoding<'a>) -> TokenStream {
    let name = field.name.to_string();
    let encoding = generate_encoding(&field.encoding);
    quote_spanned!(field.name.span()=> tezos_encoding::encoding::Field::new(#name, #encoding))
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

fn generate_tag_encoding<'a>(tag: &Tag<'a>) -> TokenStream {
    let id = &tag.id;
    let name = tag.name.to_string();
    let encoding = generate_encoding(&tag.encoding);
    quote_spanned!(tag.name.span()=> tezos_encoding::encoding::Tag::new(#id, #name, #encoding))
}

fn generate_string_encoding(size: &Option<syn::Expr>) -> TokenStream {
    size.as_ref().map_or_else(|| quote!(tezos_encoding::encoding::Encoding::String), |size| quote!(tezos_encoding::encoding::Encoding::BoundedString(#size)))
}

fn generate_list_encoding<'a>(size: &Option<syn::Expr>, encoding: &Encoding<'a>) -> TokenStream {
    let encoding = generate_encoding(encoding);
    size.as_ref().map_or_else(|| quote!(tezos_encoding::encoding::Encoding::List(Box::new(#encoding))), |size| quote!(tezos_encoding::encoding::Encoding::BoundedList(#size, Box::new(#encoding))))
}

fn generate_sized_encoding<'a>(size: &syn::Expr, encoding: &Encoding<'a>) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote!(tezos_encoding::encoding::Encoding::Sized(#size, Box::new(#encoding)))
}

fn generate_bounded_encoding<'a>(size: &syn::Expr, encoding: &Encoding<'a>) -> TokenStream {
    let encoding = generate_encoding(encoding);
    quote!(tezos_encoding::encoding::Encoding::Bounded(#size, Box::new(#encoding)))
}

fn generate_dynamic_encoding<'a>(size: &Option<syn::Expr>, encoding: &Encoding<'a>) -> TokenStream {
    let encoding = generate_encoding(encoding);
    size.as_ref().map_or_else(|| quote!(tezos_encoding::encoding::Encoding::Dynamic(Box::new(#encoding))), |size| quote!(tezos_encoding::encoding::Encoding::BoundedDynamic(#size, Box::new(#encoding))))
}
