

pub trait NomReader : Sized {
    fn from_bytes(bytes: &[u8]) -> nom::IResult<&[u8], Self>;
}
