use crypto::hash::HashTrait;
use nom::{
    branch::*,
    bytes::complete::*,
    combinator::*,
    error::ErrorKind,
    multi::*,
    number::{complete::*, Endianness},
    sequence::*,
    Err, InputLength, Parser, Slice,
};
pub use tezos_encoding_derive::NomReader;

use self::error::{BoundedEncodingKind, DecodeError, DecodeErrorKind};

pub mod error {
    use std::{fmt::Write, str::Utf8Error};

    use nom::{
        error::{ErrorKind, FromExternalError},
        Offset,
    };

    use super::NomInput;

    /// Decoding error
    #[derive(Debug, PartialEq)]
    pub struct DecodeError<'a> {
        /// Input causing the error.
        pub(crate) input: NomInput<'a>,
        /// Kind of the error.
        pub(crate) kind: DecodeErrorKind,
        /// Subsequent error, if any.
        pub(crate) other: Option<Box<DecodeError<'a>>>,
    }

    /// Decoding error kind.
    #[derive(Debug, PartialEq)]
    pub enum DecodeErrorKind {
        /// Nom-specific error.
        Nom(ErrorKind),
        /// Error converting bytes to a UTF-8 string.
        Utf8(ErrorKind, Utf8Error),
        /// Boundary violation.
        Boundary(BoundedEncodingKind),
        /// Field name
        Field(&'static str),
        /// Field name
        Variant(&'static str),
    }

    /// Specific bounded encoding kind.
    #[derive(Debug, PartialEq, Clone)]
    pub enum BoundedEncodingKind {
        String,
        List,
        Dynamic,
        Bounded,
    }

    impl<'a> DecodeError<'a> {
        pub fn add_field(self, name: &'static str) -> Self {
            Self {
                input: self.input.clone(),
                kind: DecodeErrorKind::Field(name),
                other: Some(Box::new(self)),
            }
        }

        pub fn add_variant(self, name: &'static str) -> Self {
            Self {
                input: self.input.clone(),
                kind: DecodeErrorKind::Variant(name),
                other: Some(Box::new(self)),
            }
        }

        pub fn limit(input: NomInput<'a>, kind: BoundedEncodingKind) -> Self {
            Self {
                input,
                kind: DecodeErrorKind::Boundary(kind),
                other: None,
            }
        }
    }

    impl<'a> nom::error::ParseError<NomInput<'a>> for DecodeError<'a> {
        fn from_error_kind(input: NomInput<'a>, kind: ErrorKind) -> Self {
            Self {
                input,
                kind: DecodeErrorKind::Nom(kind),
                other: None,
            }
        }

        fn append(input: NomInput<'a>, kind: ErrorKind, other: Self) -> Self {
            Self {
                input,
                kind: DecodeErrorKind::Nom(kind),
                other: Some(Box::new(other)),
            }
        }
    }

    impl<'a> FromExternalError<NomInput<'a>, Utf8Error> for DecodeError<'a> {
        fn from_external_error(input: NomInput<'a>, kind: ErrorKind, e: Utf8Error) -> Self {
            Self {
                input,
                kind: DecodeErrorKind::Utf8(kind, e),
                other: None,
            }
        }
    }

    pub fn convert_error(input: NomInput, error: DecodeError) -> String {
        let mut res = String::new();
        let start = input.offset(error.input);
        let end = start + error.input.len();
        let _ = write!(res, "Error decoding bytes [{}..{}]", start, end);
        let _ = match error.kind {
            DecodeErrorKind::Nom(kind) => write!(res, " by nom parser `{:?}`", kind),
            DecodeErrorKind::Utf8(kind, e) => write!(res, " by nom parser `{:?}`: {}", kind, e),
            DecodeErrorKind::Boundary(kind) => {
                write!(
                    res,
                    " caused by boundary violation of encoding `{:?}`",
                    kind
                )
            }
            DecodeErrorKind::Field(name) => {
                write!(res, " while decoding field `{}`", name)
            }
            DecodeErrorKind::Variant(name) => {
                write!(res, " while decoding variant `{}`", name)
            }
        };

        if let Some(other) = error.other {
            let _ = write!(res, "\n\nNext error:\n{}", convert_error(input, *other));
        }

        res
    }
}

/// Input for decoding.
pub type NomInput<'a> = &'a [u8];

/// Error type used to parameterize `nom`.
pub type NomError<'a> = error::DecodeError<'a>;

/// Nom result used in Tezedge (`&[u8]` as input, [NomError] as error type).
pub type NomResult<'a, T> = nom::IResult<NomInput<'a>, T, NomError<'a>>;

/// Traits defining message decoding using `nom` primitives.
pub trait NomReader: Sized {
    fn from_bytes(bytes: &[u8]) -> NomResult<Self>;
}

macro_rules! hash_nom_reader {
    ($hash_name:ident) => {
        impl NomReader for crypto::hash::$hash_name {
            #[inline(always)]
            fn from_bytes(bytes: &[u8]) -> NomResult<Self> {
                map(take(Self::hash_size()), |bytes| {
                    Self::try_from_bytes(bytes).unwrap()
                })(bytes)
            }
        }
    };
}

hash_nom_reader!(ChainId);
hash_nom_reader!(BlockHash);
hash_nom_reader!(BlockMetadataHash);
hash_nom_reader!(OperationHash);
hash_nom_reader!(OperationListListHash);
hash_nom_reader!(OperationMetadataHash);
hash_nom_reader!(OperationMetadataListListHash);
hash_nom_reader!(ContextHash);
hash_nom_reader!(ProtocolHash);
hash_nom_reader!(ContractKt1Hash);
hash_nom_reader!(ContractTz1Hash);
hash_nom_reader!(ContractTz2Hash);
hash_nom_reader!(ContractTz3Hash);
hash_nom_reader!(CryptoboxPublicKeyHash);
hash_nom_reader!(PublicKeyEd25519);
hash_nom_reader!(PublicKeySecp256k1);
hash_nom_reader!(PublicKeyP256);

/// Reads a boolean value.
#[inline(always)]
pub fn boolean(input: NomInput) -> NomResult<bool> {
    alt((
        map(tag(&[crate::types::BYTE_VAL_TRUE][..]), |_| true),
        map(tag(&[crate::types::BYTE_VAL_FALSE][..]), |_| false),
    ))(input)
}

/// Reads all available bytes into a [Vec]. Used in conjunction with [sized].
#[inline(always)]
pub fn bytes(input: NomInput) -> NomResult<Vec<u8>> {
    map(rest, Vec::from)(input)
}

/// Reads size encoded as 4-bytes big-endian unsigned.
#[inline(always)]
pub fn size(input: NomInput) -> NomResult<u32> {
    u32(Endianness::Big)(input)
}

/// Reads size encoded as 4-bytes big-endian unsigned, checking that it does not exceed the `max` value.
#[inline(always)]
fn bounded_size<'a>(
    kind: BoundedEncodingKind,
    max: usize,
) -> impl FnMut(NomInput) -> NomResult<u32> {
    move |input| {
        let i = input.clone();
        let (input, size) = size(input)?;
        if size as usize <= max {
            Ok((input, size))
        } else {
            Err(Err::Error(DecodeError::limit(i, kind.clone())))
        }
    }
}

/// Reads Tesoz string encoded as a 32-bit length followed by the string bytes.
#[inline(always)]
pub fn string(input: NomInput) -> NomResult<String> {
    map_res(length_data(size), |bytes| {
        std::str::from_utf8(bytes).map(str::to_string)
    })(input)
}

/// Returns parser that reads Tesoz string encoded as a 32-bit length followed by the string bytes,
/// checking that the lengh of the string does not exceed `max`.
#[inline(always)]
pub fn bounded_string<'a>(max: usize) -> impl FnMut(NomInput<'a>) -> NomResult<'a, String> {
    map_res(
        length_data(bounded_size(BoundedEncodingKind::String, max)),
        |bytes| std::str::from_utf8(bytes).map(str::to_string),
    )
}

/// Parser that applies specified parser to the fixed length slice of input.
#[inline(always)]
pub fn sized<'a, O, F>(size: usize, f: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
{
    map_parser(take(size), f)
}

/// Parses optional field. Byte `0x00` indicates absence of the field,
/// byte `0xff` preceedes encoding of the existing field.
#[inline(always)]
pub fn optional_field<'a, O, F>(parser: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, Option<O>>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    alt((
        preceded(tag(0x00u8.to_be_bytes()), success(None)),
        preceded(tag(0xffu8.to_be_bytes()), map(parser, Some)),
    ))
}

/// Parses input by applying parser `f` to it.
#[inline(always)]
pub fn list<'a, O, F>(f: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, Vec<O>>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    fold_many0(f, Vec::new(), |mut list, item| {
        list.push(item);
        list
    })
}

/// Parses input by applying parser `f` to it no more than `max` times.
#[inline(always)]
pub fn bounded_list<'a, O, F>(
    max: usize,
    mut f: F,
) -> impl FnMut(NomInput<'a>) -> NomResult<'a, Vec<O>>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    move |input| {
        let (input, list) = fold_many_m_n(
            0,
            max,
            |i| f.parse(i),
            Vec::new(),
            |mut list, item| {
                list.push(item);
                list
            },
        )(input)?;
        if input.input_len() > 0 {
            Err(Err::Error(DecodeError {
                input,
                kind: DecodeErrorKind::Boundary(BoundedEncodingKind::List),
                other: None,
            }))
        } else {
            Ok((input, list))
        }
    }
}

/// Parses dynamic block by reading 4-bytes size and applying the parser `f` to the following sequence of bytes of that size.
#[inline(always)]
pub fn dynamic<'a, O, F>(f: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    length_value(size, all_consuming(f))
}

/// Parses dynamic block by reading 4-bytes size and applying the parser `f`
/// to the following sequence of bytes of that size. It also checks that the size
/// does not exceed the `max` value.
#[inline(always)]
pub fn bounded_dynamic<'a, O, F>(max: usize, f: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    length_value(
        bounded_size(BoundedEncodingKind::Dynamic, max),
        all_consuming(f),
    )
}

/// Applies the parser `f` to the input, limiting it to `max` bytes at most.
#[inline(always)]
pub fn bounded<'a, O, F>(max: usize, mut f: F) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
    O: Clone,
{
    move |input: NomInput| {
        let max = std::cmp::min(max, input.input_len());
        let bounded = input.slice(std::ops::RangeTo { end: max });
        match f.parse(bounded) {
            Ok((rest, parsed)) => Ok((
                input.slice(std::ops::RangeFrom {
                    start: max - rest.input_len(),
                }),
                parsed,
            )),
            Err(Err::Error(DecodeError {
                input,
                kind: error::DecodeErrorKind::Nom(ErrorKind::Eof),
                other,
            })) => Err(Err::Error(DecodeError {
                input,
                kind: error::DecodeErrorKind::Boundary(BoundedEncodingKind::Bounded),
                other,
            })),
            e => e,
        }
    }
}

/// Applies the `parser` to the input, addin field context to the error.
#[inline(always)]
pub fn field<'a, O, F>(
    name: &'static str,
    mut parser: F,
) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
{
    move |input| parser(input).map_err(|e| e.map(|e| e.add_field(name)))
}

/// Applies the `parser` to the input, addin enum variant context to the error.
#[inline(always)]
pub fn variant<'a, O, F>(
    name: &'static str,
    mut parser: F,
) -> impl FnMut(NomInput<'a>) -> NomResult<'a, O>
where
    F: FnMut(NomInput<'a>) -> NomResult<'a, O>,
{
    move |input| parser(input).map_err(|e| e.map(|e| e.add_variant(name)))
}

#[cfg(test)]
mod test {
    use super::error::*;
    use super::*;

    #[test]
    fn test_boolean() {
        let res: NomResult<bool> = boolean(&[0xff]);
        assert_eq!(res, Ok((&[][..], true)));

        let res: NomResult<bool> = boolean(&[0x00]);
        assert_eq!(res, Ok((&[][..], false)));

        let res: NomResult<bool> = boolean(&[0x01]);
        res.expect_err("Error is expected");
    }

    #[test]
    fn test_size() {
        let input = &[0xff, 0xff, 0xff, 0xff];
        let res: NomResult<u32> = size(input);
        assert_eq!(res, Ok((&[][..], 0xffffffff)))
    }

    #[test]
    fn test_bounded_size() {
        let input = &[0x00, 0x00, 0x00, 0x10];

        let res: NomResult<u32> = bounded_size(BoundedEncodingKind::String, 100)(input);
        assert_eq!(res, Ok((&[][..], 0x10)));

        let res: NomResult<u32> = bounded_size(BoundedEncodingKind::String, 0x10)(input);
        assert_eq!(res, Ok((&[][..], 0x10)));

        let res: NomResult<u32> = bounded_size(BoundedEncodingKind::String, 0xf)(input);
        let err = res.expect_err("Error is expected");
        assert_eq!(err, limit_error(input, BoundedEncodingKind::String));
    }

    #[test]
    fn test_bytes() {
        let input = &[0, 1, 2, 3];
        let res: NomResult<Vec<u8>> = bytes(input);
        assert_eq!(res, Ok((&[][..], vec![0, 1, 2, 3])))
    }

    #[test]
    fn test_optional_field() {
        let res: NomResult<Option<u8>> = optional_field(u8)(&[0x00, 0x01][..]);
        assert_eq!(res, Ok((&[0x01][..], None)));

        let res: NomResult<Option<u8>> = optional_field(u8)(&[0xff, 0x01][..]);
        assert_eq!(res, Ok((&[][..], Some(0x01))));

        let res: NomResult<Option<u8>> = optional_field(u8)(&[0x01, 0x01][..]);
        res.expect_err("Error is expected");
    }

    #[test]
    fn test_string() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];
        let res: NomResult<String> = string(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())))
    }

    #[test]
    fn test_bounded_string() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];

        let res: NomResult<String> = bounded_string(3)(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())));

        let res: NomResult<String> = bounded_string(4)(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())));

        let res: NomResult<String> = bounded_string(2)(input);
        let err = res.expect_err("Error is expected");
        assert_eq!(err, limit_error(input, BoundedEncodingKind::String));
    }

    #[test]
    fn test_sized_bytes() {
        let input = &[0, 1, 2, 3, 4, 5, 6];
        let res: NomResult<Vec<u8>> = sized(4, bytes)(input);
        assert_eq!(res, Ok((&[4, 5, 6][..], vec![0, 1, 2, 3])))
    }

    #[test]
    fn test_list() {
        let input = &[0, 1, 2, 3, 4, 5];
        let res: NomResult<Vec<u16>> = list(u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));
    }

    #[test]
    fn test_bounded_list() {
        let input = &[0, 1, 2, 3, 4, 5];

        let res: NomResult<Vec<u16>> = bounded_list(4, u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));

        let res: NomResult<Vec<u16>> = bounded_list(3, u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));

        let res: NomResult<Vec<u16>> = bounded_list(2, u16(Endianness::Big))(input);
        let err = res.expect_err("Error is expected");
        assert_eq!(err, limit_error(&input[4..], BoundedEncodingKind::List));
    }

    #[test]
    fn test_dynamic() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];

        let res: NomResult<Vec<u8>> = dynamic(bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));

        let res: NomResult<u8> = dynamic(u8)(input);
        res.expect_err("Error is expected");
    }

    #[test]
    fn test_bounded_dynamic() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];

        let res: NomResult<Vec<u8>> = bounded_dynamic(4, bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));

        let res: NomResult<Vec<u8>> = bounded_dynamic(3, bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));

        let res: NomResult<Vec<u8>> = bounded_dynamic(2, bytes)(input);
        let err = res.expect_err("Error is expected");
        assert_eq!(err, limit_error(input, BoundedEncodingKind::Dynamic));
    }

    #[test]
    fn test_bounded() {
        let input = &[1, 2, 3, 4, 5];

        let res: NomResult<Vec<u8>> = bounded(4, bytes)(input);
        assert_eq!(res, Ok((&[5][..], vec![1, 2, 3, 4])));

        let res: NomResult<Vec<u8>> = bounded(3, bytes)(input);
        assert_eq!(res, Ok((&[4, 5][..], vec![1, 2, 3])));

        let res: NomResult<Vec<u8>> = bounded(10, bytes)(input);
        assert_eq!(res, Ok((&[][..], vec![1, 2, 3, 4, 5])));

        let res: NomResult<u32> = bounded(3, u32(Endianness::Big))(input);
        let err = res.expect_err("Error is expected");
        assert_eq!(err, limit_error(&input[..3], BoundedEncodingKind::Bounded));
    }

    fn limit_error<'a>(input: NomInput<'a>, kind: BoundedEncodingKind) -> Err<NomError<'a>> {
        Err::Error(DecodeError {
            input,
            kind: DecodeErrorKind::Boundary(kind),
            other: None,
        })
    }
}
