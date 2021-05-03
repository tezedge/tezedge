

pub trait NomReader : Sized {
    fn from_bytes_nom(bytes: &[u8]) -> nom::IResult<&[u8], Self>;
}
