use crate::encoding::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

pub fn generate_nom_read_for_data<'a>(data: &DataWithEncoding<'a>) -> TokenStream {
    let name = data.name;
    let nom_read = generate_nom_read(&data.encoding);
    quote_spanned! {
        data.name.span()=>
        #[allow(unused_parens)]
        impl tezos_encoding::nom::NomReader for #name {
            fn from_bytes(bytes: &[u8]) -> nom::IResult<&[u8], Self> {
                #nom_read(bytes)
            }
        }
    }
}

fn generate_nom_read<'a>(encoding: &Encoding<'a>) -> TokenStream {
    match encoding {
        Encoding::Unit => unreachable!(),
        Encoding::Primitive(primitive) => generage_primitive_nom_read(primitive),
        Encoding::Bytes(span) => generate_bytes_nom_read(*span),
        Encoding::Path(path) => quote_spanned!(path.span()=> <#path as tezos_encoding::nom::NomReader>::from_bytes),
        Encoding::Struct(encoding) => generate_struct_nom_read(encoding),
        Encoding::Enum(encoding) => generate_enum_nom_read(encoding),
        Encoding::String(size, span) => generate_string_nom_read(size, *span),
        Encoding::List(size, encoding, span) => generate_list_nom_read(size, encoding, *span),
        Encoding::Sized(size, encoding, span) => generate_sized_nom_read(size, encoding, *span),
        Encoding::Bounded(size, encoding, span) => generate_bounded_nom_read(size, encoding, *span),
        Encoding::Dynamic(size, encoding, span) => generate_dynamic_nom_read(size, encoding, *span),
    }
}

fn generage_primitive_nom_read(ident: &syn::Ident) -> TokenStream {
    generate_number_nom_read(ident)
}

fn generate_number_nom_read(ty: &syn::Ident) -> TokenStream {
    quote_spanned!(ty.span()=> nom::number::complete::#ty(nom::number::Endianness::Big))
}

fn generate_bytes_nom_read(span: Span) -> TokenStream {
    quote_spanned!(span=> tezos_encoding::nom::bytes)
}

fn generate_struct_nom_read(encoding: &StructEncoding) -> TokenStream {
    let name = encoding.name;
    let field1 = encoding.fields.iter().map(|field| field.name);
    let field2 = field1.clone();
    let field_nom_read = encoding
        .fields
        .iter()
        .map(|field| field.encoding.as_ref().map(|encoding| generate_nom_read(&encoding)).unwrap_or_else(|| quote!(|input| Ok((input, Default::default())))));
    quote_spanned! {
        encoding.name.span()=>
        nom::combinator::map(
            nom::sequence::tuple((
                #(#field_nom_read),*
            )),
            |(#(#field1),*)| #name { #(#field2),* }
        )
    }
}

fn generate_enum_nom_read(encoding: &EnumEncoding) -> TokenStream {
    let tag_type = &encoding.tag_type;
    let tags_nom_read = encoding
        .tags
        .iter()
        .map(|tag| generate_tag_nom_read(tag, encoding.name, tag_type));
    quote_spanned! {
        tag_type.span()=>
        nom::branch::alt((
            #(#tags_nom_read),*
        ))
    }
}

fn generate_tag_nom_read<'a>(
    tag: &Tag<'a>,
    enum_name: &syn::Ident,
    tag_type: &syn::Ident,
) -> TokenStream {
    let id = &tag.id;
    let tag_name = tag.name;
    let nom_read = match &tag.encoding {
        Encoding::Unit => quote_spanned!(tag_name.span()=> |bytes| Ok((bytes, #enum_name::#tag_name))),
        encoding => {
            let nom_read = generate_nom_read(&encoding);
            quote_spanned!(tag_name.span()=> nom::combinator::map(#nom_read, #enum_name::#tag_name))
        }
    };
    quote_spanned! {
        tag.name.span()=>
        nom::sequence::preceded(
            nom::bytes::complete::tag((#id as #tag_type).to_be_bytes()),
            #nom_read
        )
    }
}

fn generate_string_nom_read(size: &Option<syn::Expr>, span: Span) -> TokenStream {
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::nom::string),
        |size| quote_spanned!(span=> tezos_encoding::nom::bounded_string(#size)),
    )
}

fn generate_list_nom_read<'a>(
    size: &Option<syn::Expr>,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let nom_read = generate_nom_read(encoding);
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::nom::list(#nom_read)),
        |size| quote_spanned!(span=> tezos_encoding::nom::bounded_list(#size, #nom_read)),
    )
}

fn generate_sized_nom_read<'a>(
    size: &syn::Expr,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let nom_read = generate_nom_read(encoding);
    quote_spanned!(span=> tezos_encoding::nom::sized(#size, #nom_read))
}

fn generate_bounded_nom_read<'a>(
    size: &syn::Expr,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let nom_read = generate_nom_read(encoding);
    quote_spanned!(span=> tezos_encoding::nom::bounded(#size, #nom_read))
}

fn generate_dynamic_nom_read<'a>(
    size: &Option<syn::Expr>,
    encoding: &Encoding<'a>,
    span: Span,
) -> TokenStream {
    let nom_read = generate_nom_read(encoding);
    size.as_ref().map_or_else(
        || quote_spanned!(span=> tezos_encoding::nom::dynamic(#nom_read)),
        |size| quote_spanned!(span=> tezos_encoding::nom::bounded_dynamic(#size, #nom_read)),
    )
}
