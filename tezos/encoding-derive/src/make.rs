// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use proc_macro2::Span;
use syn::spanned::Spanned;

use crate::encoding::*;
use crate::symbol;

type Result<T> = std::result::Result<T, syn::Error>;

pub fn make_encoding(input: &syn::DeriveInput) -> Result<DataWithEncoding> {
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
            Encoding::Struct(make_struct_encoding(data_struct, name)?)
        }
        syn::Data::Enum(data_enum) => Encoding::Enum(make_enum_encoding(data_enum, name, meta)?),
        syn::Data::Union(data_union) => {
            return Err(error_spanned(
                data_union.union_token,
                "`union` is not supported",
            ))
        }
    };
    let encoding = make_bounded_encoding(meta, encoding)?;
    assert_empty_meta(meta)?;
    Ok(DataWithEncoding { name, encoding })
}

fn make_struct_encoding<'a>(
    data: &'a syn::DataStruct,
    name: &'a syn::Ident,
) -> Result<StructEncoding<'a>> {
    let fields = match &data.fields {
        syn::Fields::Named(fields_named) => make_fields(&fields_named.named)?,
        _ => {
            return Err(error_spanned(
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
    fields.into_iter().map(make_field).collect()
}

fn field_kind<'a, 'b>(meta: &'a [syn::Meta]) -> Option<FieldKind<'b>> {
    meta.iter().find_map(|meta| match meta {
        syn::Meta::Path(path) if path == symbol::SKIP => Some(FieldKind::Skip),
        syn::Meta::Path(path) if path == symbol::HASH => Some(FieldKind::Hash),
        _ => None,
    })
}

fn make_field(field: &syn::Field) -> Result<FieldEncoding> {
    let meta = &mut get_encoding_meta(&field.attrs)?;
    let name = field.ident.as_ref().unwrap();
    let kind = field_kind(meta);
    let kind = match kind {
        Some(kind) => kind,
        None => {
            let encoding = make_type_encoding(&field.ty, meta)?;
            let encoding = make_bounded_encoding(meta, encoding)?;
            let reserve = get_attribute_with_param(meta, &symbol::RESERVE, None, true)?;
            assert_empty_meta(meta)?;
            FieldKind::Encoded(Box::new(EncodedField {
                encoding,
                reserve: reserve.map(|r| r.param),
            }))
        }
    };
    Ok(FieldEncoding { name, kind })
}

/// Creates encoding from the type `ty` and meta attributes.
fn make_type_encoding<'a>(ty: &'a syn::Type, meta: &mut Vec<syn::Meta>) -> Result<Encoding<'a>> {
    match ty {
        syn::Type::Path(type_path) => make_type_path_encoding(&type_path.path, meta),
        _ => Err(error_spanned(ty, "Unsupported type")),
    }
}

/// Creates encoding from the type path `ty` (e.g. `mod::ty` or `u8`) and meta attributes.
fn make_type_path_encoding<'a>(
    path: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
) -> Result<Encoding<'a>> {
    let segment = path.segments.last().unwrap();
    match &segment.arguments {
        syn::PathArguments::None => make_basic_encoding_from_type(path, meta),
        syn::PathArguments::AngleBracketed(args) => {
            let encoding = if segment.ident == symbol::rust::VEC {
                let encoding = type_argument_encoding(args, meta)?;
                make_list_encoding_from_type(path, meta, encoding)?
            } else if segment.ident == symbol::rust::OPTION {
                let encoding = type_argument_encoding(args, meta)?;
                make_optional_field_encoding_from_type(path, meta, encoding)?
            } else {
                make_basic_encoding_from_type(path, meta)?
            };
            let encoding = make_bounded_encoding(meta, encoding)?;
            Ok(encoding)
        }
        _ => Err(error_spanned(
            &segment.arguments,
            "Only angle-bracketed generic arguments are supported",
        )),
    }
}

fn type_argument_encoding<'a>(
    args: &'a syn::AngleBracketedGenericArguments,
    meta: &mut Vec<syn::Meta>,
) -> Result<Encoding<'a>> {
    if args.args.len() != 1 {
        Err(error_spanned(&args.args, "Expected single argument"))
    } else {
        let arg = args.args.last().unwrap();
        match arg {
            syn::GenericArgument::Type(ty) => make_type_encoding(ty, meta),
            _ => Err(error_spanned(arg, "Only type generic parameters supported")),
        }
    }
}

/// Constructs encoding for a non-parameterized type.
fn make_basic_encoding_from_type<'a>(
    path: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
) -> Result<Encoding<'a>> {
    let ident = &path.segments.last().unwrap().ident;
    let encoding = if ident == symbol::rust::STRING {
        // String type is mapped to String encoding.
        let string_attr =
            get_attribute_with_option(meta, &symbol::STRING, Some(&symbol::MAX), true)?;
        Encoding::String(string_attr.map(|param| param.param).flatten(), ident.span())
    } else if ident == symbol::rust::I64 && has_attribute(meta, &symbol::TIMESTAMP) {
        if let Some(timestamp) = get_attribute_no_param(meta, &symbol::TIMESTAMP)? {
            Encoding::Primitive(PrimitiveEncoding::Timestamp, timestamp.span)
        } else {
            unreachable!()
        }
    } else if let Some(mapped) = get_rust_to_primitive_mapping(ident) {
        // direct mapping from Rust type to encoding
        assert_builtin_encoding(meta, &mapped)?;
        Encoding::Primitive(mapped, ident.span())
    } else if let Some(builtin) =
        get_attribute_with_param(meta, &symbol::BUILTIN, Some(&symbol::KIND), true)?
    {
        // Built-in encoding is specified.
        Encoding::Primitive(builtin.param, builtin.span)
    } else if let Some(meta) = get_composite_meta(meta)? {
        // return immediately to not consume other meta attributes
        // TODO: check this
        return make_composite_encoding(path, meta);
    } else {
        Encoding::Path(path)
    };
    let encoding = make_bounded_encoding(meta, encoding)?;
    Ok(encoding)
}

/// Consumes the rightmost meta attribute to make basic encoding.
fn get_basic_encoding_from_meta<'a>(
    _ty: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
) -> Result<Option<Encoding<'a>>> {
    let encoding = if let Some(bytes) = get_attribute_no_param(meta, &symbol::STRING)? {
        Encoding::Bytes(bytes.span)
    } else if let Some(string) =
        get_attribute_with_option(meta, &symbol::STRING, Some(&symbol::MAX), true)?
    {
        Encoding::String(string.param, string.span)
    } else if let Some(zarith) = get_attribute_no_param(meta, &symbol::Z_ARITH)? {
        Encoding::Zarith(zarith.span)
    } else if let Some(mutez) = get_attribute_no_param(meta, &symbol::MU_TEZ)? {
        Encoding::MuTez(mutez.span)
    } else if let Some(builtin) =
        get_attribute_with_param(meta, &symbol::BUILTIN, Some(&symbol::KIND), true)?
    {
        Encoding::Primitive(builtin.param, builtin.span)
    } else {
        return Ok(None);
    };
    Ok(Some(encoding))
}

/// Creates basic encoding by consumes the rightmost meta attribute.
fn make_basic_encoding_from_meta<'a>(
    ty: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
) -> Result<Encoding<'a>> {
    get_basic_encoding_from_meta(ty, meta)?
        .ok_or_else(|| error_spanned(ty, "No basic encoding specified"))
}

/// Returns content of the `composite` attribute.
fn get_composite_meta(meta: &mut Vec<syn::Meta>) -> Result<Option<Vec<syn::Meta>>> {
    if let Some(composite) = get_attribute(meta, &symbol::COMPOSITE) {
        if let syn::Meta::List(list) = composite {
            let list = list
                .nested
                .into_iter()
                .map(|nested| {
                    if let syn::NestedMeta::Meta(meta) = nested {
                        Ok(meta)
                    } else {
                        Err(error_spanned(nested, "Invalid nested attribute type"))
                    }
                })
                .collect::<Result<_>>()?;
            Ok(Some(list))
        } else {
            Err(error_spanned(composite, "Invalid attribute type"))
        }
    } else {
        Ok(None)
    }
}

/// Asserts that meta attribute corresponds to the specified built-in encoding `kind`.
fn assert_builtin_encoding(meta: &mut Vec<syn::Meta>, kind: &PrimitiveEncoding) -> Result<()> {
    if let Some(builtin) = get_attribute_with_param::<PrimitiveEncoding>(
        meta,
        &symbol::BUILTIN,
        Some(&symbol::KIND),
        true,
    )? {
        if *kind != builtin.param {
            return Err(error(
                builtin.span,
                "Built-in encoding does not match the type",
            ));
        }
    } else if let Some(string) = get_attribute(meta, &symbol::STRING) {
        return Err(error_spanned(
            string,
            "String encoding can be used only with `String` type",
        ));
    }
    Ok(())
}

/// Constructs encoding from the content of the `composite` meta attribute.
fn make_composite_encoding(ty: &syn::Path, mut meta: Vec<syn::Meta>) -> Result<Encoding> {
    let meta = &mut meta;
    let mut encoding = make_basic_encoding_from_meta(ty, meta)?;
    loop {
        let size = meta.len();
        encoding = make_bounded_encoding(meta, encoding)?;
        encoding = make_list_encoding_from_meta(meta, encoding)?;
        encoding = make_optional_field_encoding_from_meta(meta, encoding)?;
        if meta.len() == size {
            assert_empty_meta(meta)?;
            return Ok(encoding);
        }
    }
}

/// Consumes `list` attribute and creates `List` encoding, returning `encoding` otherwise.
fn make_list_encoding_from_meta<'a>(
    meta: &mut Vec<syn::Meta>,
    encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    let encoding = if let Some(list) =
        get_attribute_with_option(meta, &symbol::LIST, Some(&symbol::MAX), true)?
    {
        Encoding::List(list.param, Box::new(encoding), list.span)
    } else {
        encoding
    };
    Ok(encoding)
}

/// Consumes `list` attribute and creates `List` encoding, returning `encoding` otherwise.
fn make_list_encoding_from_type<'a>(
    ty: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
    encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    let encoding = if let Some(bytes_meta) = get_attribute_no_param(meta, &symbol::BYTES)? {
        if let Encoding::Primitive(PrimitiveEncoding::Uint8, _) = encoding {
            Encoding::Bytes(bytes_meta.span)
        } else {
            return Err(error_spanned(ty, "Incompatible type for `bytes` encoding"));
        }
    } else if let Some(list_meta) =
        get_attribute_with_option(meta, &symbol::LIST, Some(&symbol::MAX), true)?
    {
        Encoding::List(list_meta.param, Box::new(encoding), list_meta.span)
    } else {
        Encoding::List(None, Box::new(encoding), ty.span())
    };
    Ok(encoding)
}

/// Consumes `option` attribute and creates `OptionField` encoding, returning `encoding` otherwise.
fn make_optional_field_encoding_from_meta<'a>(
    meta: &mut Vec<syn::Meta>,
    encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    let encoding = if let Some(option) = get_attribute_no_param(meta, &symbol::OPTION)? {
        Encoding::OptionField(Box::new(encoding), option.span)
    } else {
        encoding
    };
    Ok(encoding)
}

/// Consumes `option` attribute and creates `OptionField` encoding, returning `encoding` otherwise.
fn make_optional_field_encoding_from_type<'a>(
    ty: &'a syn::Path,
    meta: &mut Vec<syn::Meta>,
    encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    let _ = get_attribute_no_param(meta, &symbol::OPTION)?;
    Ok(Encoding::OptionField(Box::new(encoding), ty.span()))
}

/// Applies bounded encodings specified in meta attributes to `encoding`.
fn make_bounded_encoding<'a>(
    meta: &mut Vec<syn::Meta>,
    mut encoding: Encoding<'a>,
) -> Result<Encoding<'a>> {
    loop {
        if meta.is_empty() {
            return Ok(encoding);
        }
        encoding = if let Some(sized) =
            get_attribute_with_param(meta, &symbol::SIZED, Some(&symbol::SIZE), true)?
        {
            Encoding::Sized(sized.param, Box::new(encoding), sized.span)
        } else if let Some(bounded) =
            get_attribute_with_param(meta, &symbol::BOUNDED, Some(&symbol::MAX), true)?
        {
            Encoding::Bounded(bounded.param, Box::new(encoding), bounded.span)
        } else if let Some(dynamic) =
            get_attribute_with_option(meta, &symbol::DYNAMIC, Some(&symbol::MAX), true)?
        {
            Encoding::Dynamic(dynamic.param, Box::new(encoding), dynamic.span)
        } else if let Some(short_dynamic) = get_attribute(meta, &symbol::SHORT_DYNAMIC) {
            Encoding::ShortDynamic(Box::new(encoding), short_dynamic.span())
        } else {
            return Ok(encoding);
        };
    }
}

/// Attribute parameter and span.
///
/// ```none
/// #[encoding(list = "MAX")]
/// parameter----------^^^
/// span-------^^^^^^^^^^^^
/// ```
struct AttrWithParam<T> {
    param: T,
    span: Span,
}

/// Gets attribute named `name` with an optional parameter named `attr`.
fn get_attribute_with_option<'a, T: syn::parse::Parse>(
    meta: &'a mut Vec<syn::Meta>,
    name: &symbol::Symbol,
    attr: Option<&symbol::Symbol>,
    is_default: bool,
) -> Result<Option<AttrWithParam<Option<T>>>> {
    get_attribute(meta, name)
        .map(|meta| {
            let param = get_value_parsed(&meta, attr, is_default)?;
            Ok(AttrWithParam {
                param,
                span: meta.span(),
            })
        })
        .transpose()
}

/// Gets attribute named `name` with a mandatory parameter named `attr`.
fn get_attribute_with_param<'a, T: syn::parse::Parse>(
    meta: &'a mut Vec<syn::Meta>,
    name: &symbol::Symbol,
    attr: Option<&symbol::Symbol>,
    is_default: bool,
) -> Result<Option<AttrWithParam<T>>> {
    get_attribute(meta, name)
        .map(|meta| {
            if let Some(param) = get_value_parsed(&meta, attr, is_default)? {
                Ok(AttrWithParam {
                    param,
                    span: meta.span(),
                })
            } else {
                Err(error_spanned(meta, "Parameter is required"))
            }
        })
        .transpose()
}

/// Gets attribute named `name` checking that it does not have any parameters.
fn get_attribute_no_param(
    meta: &mut Vec<syn::Meta>,
    name: &symbol::Symbol,
) -> Result<Option<AttrWithParam<()>>> {
    get_attribute(meta, name)
        .map(|meta| {
            if let syn::Meta::Path(_) = meta {
                Ok(AttrWithParam {
                    param: (),
                    span: meta.span(),
                })
            } else {
                Err(error_spanned(meta, "Parameter is unexpected"))
            }
        })
        .transpose()
}

fn make_enum_encoding<'a>(
    data: &'a syn::DataEnum,
    name: &'a syn::Ident,
    meta: &mut Vec<syn::Meta>,
) -> Result<EnumEncoding<'a>> {
    let ignore_unknown = get_attribute_no_param(meta, &symbol::IGNORE_UNKNOWN)?.is_some();
    let tag_type = get_attribute_value_parsed(meta, &symbol::TAGS)?
        .unwrap_or_else(|| syn::Ident::new("u8", data.enum_token.span()));
    let tags = make_tags(&data.variants)?;
    Ok(EnumEncoding {
        name,
        tag_type,
        ignore_unknown,
        tags,
    })
}

fn make_tags<'a>(variants: impl IntoIterator<Item = &'a syn::Variant>) -> Result<Vec<Tag<'a>>> {
    let mut default_id = 0;
    let mut tags = Vec::new();
    for variant in variants {
        let meta = &mut get_encoding_meta(&variant.attrs)?;
        let tag = make_tag(variant, meta, &mut default_id)?;
        tags.push(tag);
    }
    Ok(tags)
}

fn make_tag<'a>(
    variant: &'a syn::Variant,
    meta: &mut Vec<syn::Meta>,
    default_id: &mut u16,
) -> Result<Tag<'a>> {
    let id = get_attribute_value(meta, &symbol::TAG)?
        .map(|lit| {
            if let syn::Lit::Int(int_lit) = lit {
                int_lit.base10_parse().map_err(|err| {
                    error_spanned(
                        &int_lit,
                        format!("cannot parse {} as integer: {}", &int_lit, err),
                    )
                })
            } else {
                Err(error_spanned(lit, "Integer literal expected"))
            }
        })
        .unwrap_or_else(|| Ok(*default_id))?;
    *default_id = id + 1;
    let name = &variant.ident;
    let encoding = match &variant.fields {
        syn::Fields::Named(_) => {
            return Err(error_spanned(
                variant,
                "Named fields are not supported for enums",
            ))
        }
        syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
            let ty = &fields.unnamed.first().unwrap().ty;
            match ty {
                syn::Type::Path(type_path) => Encoding::Path(&type_path.path),
                _ => return Err(error_spanned(ty, "Unsupported type for enum variant")),
            }
        }
        syn::Fields::Unnamed(fields) => {
            return Err(error_spanned(fields, "Only single field is supported"))
        }
        syn::Fields::Unit => Encoding::Unit,
    };
    Ok(Tag {
        id: syn::LitInt::new(&id.to_string(), variant.span()),
        name,
        encoding,
    })
}

fn assert_empty_meta(meta: &[syn::Meta]) -> Result<()> {
    if let Some(attr) = meta.last() {
        Err(error_spanned(attr, "Unrecognized attribute"))
    } else {
        Ok(())
    }
}

fn has_attribute(meta: &mut Vec<syn::Meta>, name: &symbol::Symbol) -> bool {
    match meta.last() {
        Some(syn::Meta::Path(path)) if path == *name => true,
        Some(syn::Meta::NameValue(name_value)) if name_value.path == *name => true,
        Some(syn::Meta::List(list)) if list.path == *name => true,
        _ => false,
    }
}

fn get_attribute(meta: &mut Vec<syn::Meta>, name: &symbol::Symbol) -> Option<syn::Meta> {
    match meta.last() {
        Some(syn::Meta::Path(path)) if path == *name => meta.pop(),
        Some(syn::Meta::NameValue(name_value)) if name_value.path == *name => meta.pop(),
        Some(syn::Meta::List(list)) if list.path == *name => meta.pop(),
        _ => None,
    }
}

fn get_value<'a>(
    meta: &'a syn::Meta,
    _attr: Option<&symbol::Symbol>,
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
    name: Option<&symbol::Symbol>,
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
        _ => Err(error_spanned(lit, "Non-string literal")),
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
                                _ => Err(error_spanned(nested, "Unexpected literal")),
                            })
                            .collect()
                    } else {
                        Err(error_spanned(meta, "Attribute list is expected"))
                    }
                }))
            } else {
                None
            }
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

lazy_static::lazy_static! {
    pub static ref RUST_TO_PRIMITIVE_MAPPING: Vec<(symbol::Symbol, PrimitiveEncoding)> = {
        use crate::symbol::rust::*;
        use crate::encoding::PrimitiveEncoding::*;
        vec![
            (I8, Int8),
            (U8, Uint8),
            (I16, Int16),
            (U16, Uint16),
            (I32, Int32),
            (U32, Uint32),
            (I64, Int64),
            (F64, Float),
            (BOOL, Bool),
        ]
    };
}

/// Returns [PrimitiveEncoding] instance corresponding to the type `ident`.
fn get_rust_to_primitive_mapping(ident: &syn::Ident) -> Option<PrimitiveEncoding> {
    RUST_TO_PRIMITIVE_MAPPING
        .iter()
        .find_map(|(i, p)| if ident.eq(i) { Some(*p) } else { None })
}

fn error_spanned(tokens: impl quote::ToTokens, message: impl std::fmt::Display) -> syn::Error {
    syn::Error::new_spanned(tokens, message)
}

fn error(span: Span, message: impl std::fmt::Display) -> syn::Error {
    syn::Error::new(span, message)
}
