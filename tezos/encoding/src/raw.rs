use crate::binary_reader::BinaryReaderError;

pub trait RawReader: Sized {
    fn from_bytes(bytes: &[u8]) -> Result<(&[u8], Self), BinaryReaderError>;
}
