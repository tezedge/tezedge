use crate::encoding::*;
use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

const NOM_TUPLE_MAX: usize = 26;

pub fn generate_nom_read_for_data<'a>(data: &DataWithEncoding<'a>) -> TokenStream {
    let name = data.name;
    let nom_read = generate_nom_read(&data.encoding);
    quote_spanned! {
        data.name.span()=>
        #[allow(unused_parens)]
        impl tezos_encoding::nom::NomReader for #name {
            fn from_bytes(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
                #nom_read(bytes)
            }
        }
    }
}

fn generate_nom_read<'a>(encoding: &Encoding<'a>) -> TokenStream {
    match encoding {
        Encoding::Unit => unreachable!(),
        Encoding::Primitive(primitive, span) => generage_primitive_nom_read(*primitive, *span),
        Encoding::Bytes(span) => generate_bytes_nom_read(*span),
        Encoding::Path(path) => {
            quote_spanned!(path.span()=> <#path as tezos_encoding::nom::NomReader>::from_bytes)
        }
        Encoding::Struct(encoding) => generate_struct_nom_read(encoding),
        Encoding::Enum(encoding) => generate_enum_nom_read(encoding),
        Encoding::String(size, span) => generate_string_nom_read(size, *span),
        Encoding::OptionField(encoding, span) => generate_optional_field_nom_read(encoding, *span),
        Encoding::List(size, encoding, span) => generate_list_nom_read(size, encoding, *span),
        Encoding::Sized(size, encoding, span) => generate_sized_nom_read(size, encoding, *span),
        Encoding::Bounded(size, encoding, span) => generate_bounded_nom_read(size, encoding, *span),
        Encoding::Dynamic(size, encoding, span) => generate_dynamic_nom_read(size, encoding, *span),
        Encoding::Z(span) => quote_spanned!(*span=> tezos_encoding::nom::zarith),
        Encoding::MuTez(span) => quote_spanned!(*span=> tezos_encoding::nom::mutez),
    }
}

lazy_static::lazy_static! {
    static ref PRIMITIVE_BYTES_MAPPING: Vec<(PrimitiveEncoding, &'static str)> = {
        use crate::encoding::PrimitiveEncoding::*;
        vec![
            (Int8, "i8"),
            (Uint8, "u8"),
        ]
    };

    static ref PRIMITIVE_NUMBERS_MAPPING: Vec<(PrimitiveEncoding, &'static str)> = {
        use crate::encoding::PrimitiveEncoding::*;
        vec![
            (Int16, "i16"),
            (Uint16, "u16"),
            (Int31, "i32"),
            (Int32, "i32"),
            (Uint32, "u32"),
            (Int64, "i64"),
            (Float, "f64"),
            (Timestamp, "i64"),
        ]
    };
}

fn get_primitive_byte_mapping(kind: PrimitiveEncoding) -> Option<&'static str> {
    PRIMITIVE_BYTES_MAPPING
        .iter()
        .find_map(|(k, s)| if kind == *k { Some(*s) } else { None })
}

fn get_primitive_number_mapping(kind: PrimitiveEncoding) -> Option<&'static str> {
    PRIMITIVE_NUMBERS_MAPPING
        .iter()
        .find_map(|(k, s)| if kind == *k { Some(*s) } else { None })
}

fn generage_primitive_nom_read(kind: PrimitiveEncoding, span: Span) -> TokenStream {
    match kind {
        PrimitiveEncoding::Int8 | PrimitiveEncoding::Uint8 => {
            generate_byte_nom_read(get_primitive_byte_mapping(kind).unwrap(), span)
        }
        PrimitiveEncoding::Int16
        | PrimitiveEncoding::Uint16
        | PrimitiveEncoding::Int31
        | PrimitiveEncoding::Int32
        | PrimitiveEncoding::Uint32
        | PrimitiveEncoding::Int64
        | PrimitiveEncoding::Float
        | PrimitiveEncoding::Timestamp => {
            generate_number_nom_read(get_primitive_number_mapping(kind).unwrap(), span)
        }
        PrimitiveEncoding::Bool => quote_spanned!(span=> tezos_encoding::nom::boolean),
    }
}

fn generate_byte_nom_read(num: &str, span: Span) -> TokenStream {
    let ty = syn::Ident::new(num, span);
    quote_spanned!(span=> nom::number::complete::#ty)
}

fn generate_number_nom_read(num: &str, span: Span) -> TokenStream {
    let ty = syn::Ident::new(num, span);
    quote_spanned!(span=> nom::number::complete::#ty(nom::number::Endianness::Big))
}

fn generate_bytes_nom_read(span: Span) -> TokenStream {
    quote_spanned!(span=> tezos_encoding::nom::bytes)
}

fn generate_struct_nom_read(encoding: &StructEncoding) -> TokenStream {
    let generate_nom_read = match encoding.fields.len() {
        0 => generate_struct_no_fields_nom_read,
        1 => generate_struct_one_field_nom_read,
        n if n < NOM_TUPLE_MAX => generate_struct_many_fields_nom_read,
        _  => generate_struct_multi_fields_nom_read,
    };
    generate_nom_read(encoding)
}

fn generate_struct_no_fields_nom_read(encoding: &StructEncoding) -> TokenStream {
    let name = encoding.name;
    quote_spanned!(encoding.name.span()=> nom::combinator::success(#name {}))
}

fn generate_struct_one_field_nom_read(encoding: &StructEncoding) -> TokenStream {
    let name = encoding.name;
    let field = encoding.fields.first().unwrap();
    let field_name = field.name;
    let field_name_str = field_name.to_string();
    let field_nom_read = generate_struct_field_nom_read(field);
    quote_spanned!(encoding.name.span()=> nom::combinator::map(tezos_encoding::nom::field(#field_name_str, #field_nom_read), |#field_name| #name { #field_name }))
}

fn generate_struct_many_fields_nom_read(encoding: &StructEncoding) -> TokenStream {
    let name = encoding.name;
    let field1 = encoding.fields.iter().map(|field| field.name);
    let field2 = field1.clone();
    let field_name = encoding
        .fields
        .iter()
        .map(|field| format!("{}::{}", name, field.name.to_string()));
    let field_nom_read = encoding.fields.iter().map(generate_struct_field_nom_read);
     quote_spanned! {
         encoding.name.span()=>
         nom::combinator::map(
             nom::sequence::tuple((
                #(tezos_encoding::nom::field(#field_name, #field_nom_read)),*
             )),
             |(#(#field1),*)| #name { #(#field2),* }
         )
     }
}

fn generate_struct_multi_fields_nom_read(encoding: &StructEncoding) -> TokenStream {
    let name = encoding.name;
    let field1 = encoding.fields.iter().map(|field| field.name);
    let field2 = field1.clone();
    let field_name = encoding
        .fields
        .iter()
        .map(|field| format!("{}::{}", name, field.name.to_string()));
    let field_nom_read = encoding.fields.iter().map(generate_struct_field_nom_read);
    quote_spanned! {
        encoding.name.span()=>
            (|input| {
                #(let (input, #field1) = tezos_encoding::nom::field(#field_name, #field_nom_read)(input)?;)*
                Ok((input, #name { #(#field2),* }))
            })
    }
}

fn generate_struct_field_nom_read(field: &FieldEncoding) -> TokenStream {
    field
        .encoding
        .as_ref()
        .map(|encoding| generate_nom_read(encoding))
        .unwrap_or_else(|| quote!(|input| Ok((input, Default::default()))))
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
        Encoding::Unit => {
            quote_spanned!(tag_name.span()=> |bytes| Ok((bytes, #enum_name::#tag_name)))
        }
        encoding => {
            let nom_read = generate_nom_read(&encoding);
            let name = format!("{}::{}", enum_name, tag_name);
            quote_spanned!(tag_name.span()=> nom::combinator::map(tezos_encoding::nom::variant(#name, #nom_read), #enum_name::#tag_name))
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

fn generate_optional_field_nom_read<'a>(encoding: &Encoding<'a>, span: Span) -> TokenStream {
    let nom_read = generate_nom_read(encoding);
    quote_spanned!(span=> tezos_encoding::nom::optional_field(#nom_read))
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
