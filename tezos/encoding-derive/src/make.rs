use syn::spanned::Spanned;

use crate::common::Result;
use crate::encoding::*;
use crate::symbol;

pub fn make_encoding<'a>(input: &'a syn::DeriveInput) -> Result<DataWithEncoding<'a>> {
    let meta = &mut get_encoding_meta(&input.attrs)?;
    let data_with_encoding = make_data_with_encoding(&input.data, &input.ident, meta)?;
    Ok(data_with_encoding)
}

fn make_data_with_encoding<'a>(
    data: &'a syn::Data,
    name: &'a syn::Ident,
    meta: &mut Vec<syn::Meta>,
) -> Result<DataWithEncoding<'a>> {
    let encoding = match data {
        syn::Data::Struct(data_struct) => {
            Encoding::Struct(make_struct_encoding(data_struct, name, meta)?)
        }
        syn::Data::Enum(data_enum) => Encoding::Enum(make_enum_encoding(data_enum, name, meta)?),
        syn::Data::Union(data_union) => {
            return Err(error(data_union.union_token, "`union` is not supported"))
        }
    };
    let encoding = add_encoding_bounds(meta, encoding)?;
    assert_empty_meta(meta)?;
    Ok(DataWithEncoding { name, encoding })
}

fn make_struct_encoding<'a>(
    data: &'a syn::DataStruct,
    name: &'a syn::Ident,
    _meta: &mut Vec<syn::Meta>,
) -> Result<StructEncoding<'a>> {
    let fields = match &data.fields {
        syn::Fields::Named(fields_named) => make_fields(&fields_named.named)?,
        _ => {
            return Err(error(
                &data.fields,
                "Only structures with named fields supported",
            ))
        }
    };
    Ok(StructEncoding { name, fields })
}

fn make_fields<'a>(
    fields: impl IntoIterator<Item = &'a syn::Field>,
) -> Result<Vec<FieldEncoding<'a>>> {
    let mut res = Vec::new();
    for field in fields {
        res.push(make_field(field)?);
    }
    Ok(res)
}

fn make_field<'a>(field: &'a syn::Field) -> Result<FieldEncoding<'a>> {
    let meta = &mut get_encoding_meta(&field.attrs)?;
    let name = field.ident.as_ref().unwrap();
    let encoding = make_type_encoding(&field.ty, meta)?;
    let encoding = add_encoding_bounds(meta, encoding)?;
    assert_empty_meta(meta)?;
    Ok(FieldEncoding { name, encoding })
}

fn make_type_encoding<'a>(ty: &'a syn::Type, meta: &mut Vec<syn::Meta>) -> Result<Encoding<'a>> {
    match ty {
        syn::Type::Path(type_path) => make_type_path_encoding(&type_path.path, meta),
        _ => Err(error(ty, "Unsupported type")),
    }
}

fn make_type_path_encoding<'a>(
    path: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
) -> Result<Encoding<'a>> {
    let segment = path.segments.last().unwrap();
    let encoding = if segment.ident == symbol::rust::VEC {
        // `Vec<something>`
        let element_encoding = match &segment.arguments {
            syn::PathArguments::AngleBracketed(args) => {
                if args.args.len() != 1 {
                    return Err(error(&args.args, "Expected single argument"));
                }
                let arg = args.args.last().unwrap();
                match arg {
                    syn::GenericArgument::Type(ty) => make_type_encoding(ty, meta)?,
                    _ => return Err(error(arg, "Only type generic parameters supported")),
                }
            }
            _ => {
                return Err(error(
                    &segment.arguments,
                    "Only angle-bracketed generic arguments are supported",
                ))
            }
        };
        match element_encoding {
            Encoding::Primitive(ident) if ident == symbol::rust::U8 => {
                // `Vec<u8>` is encoded as bytes
                let meta = get_attribute(meta, &symbol::BYTES)?;
                let span = meta
                    .as_ref()
                    .map(Spanned::span)
                    .unwrap_or_else(|| segment.span());
                Encoding::Bytes(span)
            }
            _ => {
                let list_meta = get_attribute(meta, &symbol::LIST)?;
                let span = list_meta
                    .as_ref()
                    .map(Spanned::span)
                    .unwrap_or_else(|| segment.ident.span());
                let size = list_meta
                    .as_ref()
                    .map(|meta| get_value_parsed(meta, &symbol::MAX, true))
                    .transpose()?
                    .flatten();
                Encoding::List(size, Box::new(element_encoding), span)
            }
        }
    } else if segment.ident == symbol::rust::STRING {
        let string_meta = get_attribute(meta, &symbol::STRING)?;
                let span = string_meta
                    .as_ref()
                    .map(Spanned::span)
                    .unwrap_or_else(|| segment.ident.span());
                let size = string_meta
                    .as_ref()
                    .map(|meta| get_value_parsed(meta, &symbol::MAX, true))
                    .transpose()?
                    .flatten();
        Encoding::String(size, span)
    } else {
        if symbol::PRIMITIVE_MAPPING.contains_key(segment.ident.to_string().as_str()) {
            Encoding::Primitive(&segment.ident)
        } else {
            Encoding::Path(path)
        }
    };
    let encoding = add_encoding_bounds(meta, encoding)?;
    Ok(encoding)
}

fn make_enum_encoding<'a>(
    data: &'a syn::DataEnum,
    name: &'a syn::Ident,
    meta: &mut Vec<syn::Meta>,
) -> Result<EnumEncoding<'a>> {
    let tag_type = get_attribute_value_parsed(meta, &symbol::TAGS)?
        .unwrap_or_else(|| syn::Ident::new("u8", data.enum_token.span()));
    let tags = make_tags(&data.variants)?;
    Ok(EnumEncoding {
        name,
        tag_type,
        tags,
    })
}

fn make_tags<'a>(variants: impl IntoIterator<Item = &'a syn::Variant>) -> Result<Vec<Tag<'a>>> {
    let mut default_id = 0;
    let mut tags = Vec::new();
    for variant in variants {
        let tag = make_tag(variant, default_id)?;
        default_id = default_id + 1;
        tags.push(tag);
    }
    Ok(tags)
}

fn make_tag(variant: &syn::Variant, default_id: u16) -> Result<Tag> {
    let meta = &mut get_encoding_meta(&variant.attrs)?;
    let id = get_attribute_value(meta, &symbol::TAG)?
        .map(|lit| {
            if let syn::Lit::Int(int_lit) = lit {
                Ok(int_lit)
            } else {
                Err(error(lit, "Integer literal expected"))
            }
        })
        .unwrap_or_else(|| Ok(syn::LitInt::new(&default_id.to_string(), variant.span())))?;
    let name = &variant.ident;
    let encoding = match &variant.fields {
        syn::Fields::Named(_) => {
            return Err(error(variant, "Named fields are not supported for enums"))
        }
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed.first().unwrap().ty;
            match ty {
                syn::Type::Path(type_path) => Encoding::Path(&type_path.path),
                _ => return Err(error(ty, "Unsupported type for enum variant")),
            }
        }
        syn::Fields::Unnamed(fields) => {
            return Err(error(fields, "Only single field is supported"))
        }
        syn::Fields::Unit => Encoding::Unit,
    };
    Ok(Tag { id, name, encoding })
}

fn add_encoding_bounds<'a>(
    meta: &mut Vec<syn::Meta>,
    encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    let mut encoding = encoding;
    while let Some(attr) = meta.pop() {
        let span = attr.span();
        encoding = match attr {
            syn::Meta::Path(path) if path == symbol::DYNAMIC => {
                Encoding::Dynamic(None, Box::new(encoding), span)
            }
            syn::Meta::NameValue(syn::MetaNameValue { path, lit, .. }) if path == symbol::SIZED => {
                Encoding::Sized(parse_value(&lit)?, Box::new(encoding), span)
            }
            syn::Meta::NameValue(syn::MetaNameValue { path, lit, .. })
                if path == symbol::BOUNDED =>
            {
                Encoding::Bounded(parse_value(&lit)?, Box::new(encoding), span)
            }
            syn::Meta::NameValue(syn::MetaNameValue { path, lit, .. })
                if path == symbol::DYNAMIC =>
            {
                Encoding::Dynamic(Some(parse_value(&lit)?), Box::new(encoding), span)
            }
            _ => {
                meta.push(attr);
                return Ok(encoding);
            }
        };
    }
    Ok(encoding)
}

fn assert_empty_meta(meta: &Vec<syn::Meta>) -> Result<()> {
    if let Some(attr) = meta.last() {
        Err(error(attr, "Unrecognized attribute"))
    } else {
        Ok(())
    }
}

fn get_attribute(meta: &mut Vec<syn::Meta>, name: &symbol::Symbol) -> Result<Option<syn::Meta>> {
    match meta.last() {
        Some(syn::Meta::Path(path)) if path == *name => Ok(meta.pop()),
        Some(syn::Meta::NameValue(name_value)) if name_value.path == *name => Ok(meta.pop()),
        Some(syn::Meta::List(list)) if list.path == *name => Ok(meta.pop()),
        _ => Ok(None),
    }
}

fn get_value<'a>(
    meta: &'a syn::Meta,
    _attr: &symbol::Symbol,
    is_default: bool,
) -> Result<Option<&'a syn::Lit>> {
    match meta {
        syn::Meta::Path(_) => Ok(None),
        syn::Meta::NameValue(name_value) if is_default => Ok(Some(&name_value.lit)),
        _ => Ok(None),
    }
}

fn get_value_parsed<T: syn::parse::Parse>(
    meta: &syn::Meta,
    name: &symbol::Symbol,
    is_default: bool,
) -> Result<Option<T>> {
    let lit = get_value(meta, name, is_default)?;
    lit.as_ref().map(|lit| parse_value(lit)).transpose()
}

fn get_attribute_value(
    meta: &mut Vec<syn::Meta>,
    name: &symbol::Symbol,
) -> Result<Option<syn::Lit>> {
    match meta.pop() {
        Some(syn::Meta::NameValue(name_value)) if name_value.path == *name => {
            Ok(Some(name_value.lit))
        }
        Some(m) => {
            meta.push(m);
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn get_attribute_value_parsed<T: syn::parse::Parse>(
    meta: &mut Vec<syn::Meta>,
    name: &symbol::Symbol,
) -> Result<Option<T>> {
    let lit = get_attribute_value(meta, name)?;
    lit.as_ref().map(|lit| parse_value(lit)).transpose()
}

fn parse_value<T: syn::parse::Parse>(lit: &syn::Lit) -> Result<T> {
    match lit {
        syn::Lit::Str(lit_str) => lit_str.parse(),
        _ => Err(error(lit, "Non-string literal")),
    }
}

/// Finds `encoding` attribute and parse its content
fn get_encoding_meta<'a>(
    attrs: impl IntoIterator<Item = &'a syn::Attribute>,
) -> Result<Vec<syn::Meta>> {
    attrs
        .into_iter()
        .find_map(|attr| {
            if attr.path == symbol::ENCODING {
                Some(attr.parse_meta().and_then(|meta| {
                    if let syn::Meta::List(meta_list) = meta {
                        meta_list
                            .nested
                            .into_iter()
                            .map(|nested| match nested {
                                syn::NestedMeta::Meta(meta) => Ok(meta),
                                _ => Err(error(nested, "Unexpected literal")),
                            })
                            .collect()
                    } else {
                        Err(error(meta, "Attribute list is expected"))
                    }
                }))
            } else {
                None
            }
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn error(tokens: impl quote::ToTokens, message: impl std::fmt::Display) -> syn::Error {
    syn::Error::new_spanned(tokens, message)
}
