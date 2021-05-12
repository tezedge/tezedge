use std::{
    ops::{RangeFrom, RangeTo},
    str::Utf8Error,
};

use crypto::hash::HashTrait;
use nom::{
    branch::*,
    bytes::complete::*,
    combinator::*,
    error::{FromExternalError, ParseError},
    multi::*,
    number::{complete::*, Endianness},
    sequence::*,
    IResult, InputIter, InputLength, InputTake, Offset, Parser, Slice,
};
pub use tezos_encoding_derive::NomReader;

pub type NomResult<'a, T> = nom::IResult<&'a [u8], T>;

/// Traits defining message decoding using `nom` primitives.
pub trait NomReader: Sized {
    fn from_bytes(bytes: &[u8]) -> NomResult<Self>;
}

macro_rules! hash_nom_reader {
	($hash_name:ident) => {
        impl NomReader for crypto::hash::$hash_name {
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
#[inline]
pub fn boolean<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], bool, E>
where
    E: ParseError<&'a [u8]>,
{
    alt((
        map(tag(&[crate::types::BYTE_VAL_TRUE][..]), |_| true),
        map(tag(&[crate::types::BYTE_VAL_FALSE][..]), |_| false),
    ))(input)
}

/// Reads all available bytes into a [Vec]. Used in conjunction with [sized].
#[inline]
pub fn bytes<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], Vec<u8>, E>
where
    E: ParseError<&'a [u8]>,
{
    map(rest, Vec::from)(input)
}

/// Reads size encoded as 4-bytes big-endian unsigned.
#[inline]
fn size<I, E>(input: I) -> IResult<I, u32, E>
where
    I: InputLength + InputIter<Item = u8> + Slice<RangeFrom<usize>>,
    E: ParseError<I>,
{
    u32(Endianness::Big)(input)
}

/// Reads size encoded as 4-bytes big-endian unsigned, checking that it does not exceed the `max` value.
#[inline]
fn bounded_size<I, E>(max: usize) -> impl FnMut(I) -> IResult<I, u32, E>
where
    I: Clone + InputLength + InputIter<Item = u8> + Slice<RangeFrom<usize>>,
    E: ParseError<I>,
{
    verify(size, move |m| (*m as usize) <= max)
}

/// Reads Tesoz string encoded as a 32-bit length followed by the string bytes.
#[inline]
pub fn string<'a, E>(input: &'a [u8]) -> IResult<&'a [u8], String, E>
where
    E: ParseError<&'a [u8]> + FromExternalError<&'a [u8], Utf8Error>,
{
    map_res(length_data(size), |bytes| {
        std::str::from_utf8(bytes).map(str::to_string)
    })(input)
}

/// Returns parser that reads Tesoz string encoded as a 32-bit length followed by the string bytes,
/// checking that the lengh of the string does not exceed `max`.
#[inline]
pub fn bounded_string<'a, E>(max: usize) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], String, E>
where
    E: ParseError<&'a [u8]> + FromExternalError<&'a [u8], Utf8Error>,
{
    map_res(length_data(bounded_size(max)), |bytes| {
        std::str::from_utf8(bytes).map(str::to_string)
    })
}

/// Parser that applies specified parser to the fixed length slice of input.
#[inline]
pub fn sized<I, O, E, F>(size: usize, f: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    F: Parser<I, O, E>,
    I: InputLength + InputTake + InputIter<Item = u8> + Clone,
    E: ParseError<I>,
{
    map_parser(take(size), f)
}

/// Parses optional field. Byte `0x00` indicates absence of the field,
/// byte `0xff` preceedes encoding of the existing field.
#[inline]
pub fn optional_field<'a, O, E, F>(parser: F) -> impl FnMut(&'a [u8]) -> IResult<&'a [u8], Option<O>, E>
where
    F: Parser<&'a [u8], O, E>,
    O: Clone,
    E: ParseError<&'a [u8]>,
{
    alt((
        preceded(tag(0x00u8.to_be_bytes()), success(None)),
        preceded(tag(0xffu8.to_be_bytes()), map(parser, Some)),
    ))
}


/// Parses input by applying parser `f` to it.
#[inline]
pub fn list<I, O, E, F>(f: F) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
    F: Parser<I, O, E>,
    I: InputLength + InputTake + InputIter + Clone + PartialEq,
    O: Clone,
    E: ParseError<I>,
{
    fold_many0(f, Vec::new(), |mut list, item| {
        list.push(item);
        list
    })
}

/// Parses input by applying parser `f` to it no more than `max` times.
#[inline]
pub fn bounded_list<I, O, E, F>(max: usize, f: F) -> impl FnMut(I) -> IResult<I, Vec<O>, E>
where
    F: Parser<I, O, E>,
    I: InputLength + InputTake + InputIter + Clone + PartialEq,
    O: Clone,
    E: ParseError<I>,
{
    fold_many_m_n(0, max, f, Vec::new(), |mut list, item| {
        list.push(item);
        list
    })
}

/// Parses dynamic block by reading 4-bytes size and applying the parser `f` to the following sequence of bytes of that size.
#[inline]
pub fn dynamic<I, O, E, F>(f: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    F: Parser<I, O, E>,
    I: InputLength + InputTake + InputIter<Item = u8> + Slice<RangeFrom<usize>> + Clone,
    O: Clone,
    E: ParseError<I>,
{
    map_parser(flat_map(size, take), f)
}

/// Parses dynamic block by reading 4-bytes size and applying the parser `f`
/// to the following sequence of bytes of that size. It also checks that the size
/// does not exceed the `max` value.
#[inline]
pub fn bounded_dynamic<I, O, E, F>(max: usize, f: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    F: Parser<I, O, E>,
    I: InputLength + InputTake + InputIter<Item = u8> + Slice<RangeFrom<usize>> + Clone,
    O: Clone,
    E: ParseError<I>,
{
    map_parser(flat_map(bounded_size(max), take), f)
}

/// Applies the parser `f` to the input, limiting it to `max` bytes at most.
#[inline]
pub fn bounded<I, O, E, F>(max: usize, mut f: F) -> impl FnMut(I) -> IResult<I, O, E>
where
    F: Parser<I, O, E>,
    I: InputLength
        + InputTake
        + Offset
        + InputIter<Item = u8>
        + Slice<RangeFrom<usize>>
        + Slice<RangeTo<usize>>
        + Clone,
    O: Clone,
    E: ParseError<I>,
{
    move |input: I| {
        let max = std::cmp::min(max, input.input_len());
        let bounded = input.slice(std::ops::RangeTo { end: max });
        match f.parse(bounded) {
            Ok((rest, parsed)) => Ok((
                input.slice(std::ops::RangeFrom {
                    start: max - rest.input_len(),
                }),
                parsed,
            )),
            e => e,
        }
    }
}

#[cfg(test)]
mod test {
    use nom::error::Error;

    use super::*;

    #[test]
    fn test_boolean() {
        let res: NomResult<bool> = boolean(&[0xff]);
        assert_eq!(res, Ok((&[][..], true)));

        let res: NomResult<bool> = boolean(&[0x00]);
        assert_eq!(res, Ok((&[][..], false)));

        let res: NomResult<bool> = boolean(&[0x01]);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &[0x01][..],
                nom::error::ErrorKind::Tag
            )))
        );
    }

    #[test]
    fn test_size() {
        let input = &[0xff, 0xff, 0xff, 0xff];
        let res: IResult<&[u8], u32> = size(input);
        assert_eq!(res, Ok((&[][..], 0xffffffff)))
    }

    #[test]
    fn test_bounded_size() {
        let input = &[0x00, 0x00, 0x00, 0x10];
        let res: IResult<&[u8], u32> = bounded_size(100)(input);
        assert_eq!(res, Ok((&[][..], 0x10)));
        let res: IResult<&[u8], u32> = bounded_size(0x10)(input);
        assert_eq!(res, Ok((&[][..], 0x10)));
        let res: IResult<&[u8], u32> = bounded_size(0xf)(input);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &input[..],
                nom::error::ErrorKind::Verify
            )))
        );
    }

    #[test]
    fn test_bytes() {
        let input = &[0, 1, 2, 3];
        let res: IResult<&[u8], Vec<u8>> = bytes(input);
        assert_eq!(res, Ok((&[][..], vec![0, 1, 2, 3])))
    }

    #[test]
    fn test_optional_field() {
        let res: NomResult<Option<u8>> = optional_field(u8)(&[0x00, 0x01][..]);
        assert_eq!(res, Ok((&[0x01][..], None)));

        let res: NomResult<Option<u8>> = optional_field(u8)(&[0xff, 0x01][..]);
        assert_eq!(res, Ok((&[][..], Some(0x01))));

        let res = optional_field(u8)(&[0x01, 0x01][..]);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &[0x01, 0x01][..],
                nom::error::ErrorKind::Tag
            )))
        );
    }

    #[test]
    fn test_string() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];
        let res: IResult<&[u8], String> = string(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())))
    }

    #[test]
    fn test_bounded_string() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];
        let res: IResult<&[u8], String> = bounded_string(3)(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())));
        let res: IResult<&[u8], String> = bounded_string(4)(input);
        assert_eq!(res, Ok((&[0xffu8][..], "xxx".to_string())));
        let res: IResult<&[u8], String> = bounded_string(2)(input);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &input[..],
                nom::error::ErrorKind::Verify
            )))
        );
    }

    #[test]
    fn test_sized_bytes() {
        let input = &[0, 1, 2, 3, 4, 5, 6];
        let res: IResult<&[u8], Vec<u8>> = sized(4, bytes)(input);
        assert_eq!(res, Ok((&[4, 5, 6][..], vec![0, 1, 2, 3])))
    }

    #[test]
    fn test_list() {
        let input = &[0, 1, 2, 3, 4, 5];
        let res: IResult<&[u8], Vec<u16>> = list(u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));
    }

    #[test]
    #[ignore]
    fn test_bounded_list() {
        let input = &[0, 1, 2, 3, 4, 5];
        let res: IResult<&[u8], Vec<u16>> = bounded_list(4, u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));
        let res: IResult<&[u8], Vec<u16>> = bounded_list(3, u16(Endianness::Big))(input);
        assert_eq!(res, Ok((&[][..], vec![0x0001, 0x0203, 0x0405])));
        let res: IResult<&[u8], Vec<u16>> = bounded_list(2, u16(Endianness::Big))(input);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &input[..],
                nom::error::ErrorKind::Verify
            )))
        );
    }

    #[test]
    fn test_dynamic() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];
        let res: IResult<&[u8], Vec<u8>> = dynamic(bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));
    }

    #[test]
    fn test_bounded_dynamic() {
        let input = &[0, 0, 0, 3, 0x78, 0x78, 0x78, 0xff];
        let res: IResult<&[u8], Vec<u8>> = bounded_dynamic(4, bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));
        let res: IResult<&[u8], Vec<u8>> = bounded_dynamic(3, bytes)(input);
        assert_eq!(res, Ok((&[0xffu8][..], vec![0x78; 3])));
        let res: IResult<&[u8], Vec<u8>> = bounded_dynamic(2, bytes)(input);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &input[..],
                nom::error::ErrorKind::Verify
            )))
        );
    }

    #[test]
    fn test_bounded() {
        let input = &[1, 2, 3, 4, 5];

        let res: IResult<&[u8], Vec<u8>> = bounded(4, bytes)(input);
        assert_eq!(res, Ok((&[5][..], vec![1, 2, 3, 4])));

        let res: IResult<&[u8], Vec<u8>> = bounded(3, bytes)(input);
        assert_eq!(res, Ok((&[4, 5][..], vec![1, 2, 3])));

        let res: IResult<&[u8], Vec<u8>> = bounded(10, bytes)(input);
        assert_eq!(res, Ok((&[][..], vec![1, 2, 3, 4, 5])));

        let res: IResult<&[u8], u32> = bounded(3, u32(Endianness::Big))(input);
        assert_eq!(
            res,
            Err(nom::Err::Error(Error::new(
                &input[..3],
                nom::error::ErrorKind::Eof
            )))
        );
    }
}
