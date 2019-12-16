#[derive(PartialEq, Debug, Clone)]
pub struct BigInt(pub num_bigint::BigInt);

impl From<num_bigint::BigInt> for BigInt {
    fn from(from: num_bigint::BigInt) -> Self {
        BigInt(from.clone())
    }
}

impl From<BigInt> for num_bigint::BigInt {
    fn from(from: BigInt) -> Self {
        from.0.clone()
    }
}

impl From<&num_bigint::BigInt> for BigInt {
    fn from(from: &num_bigint::BigInt) -> Self {
        BigInt(from.clone())
    }
}

impl From<&BigInt> for num_bigint::BigInt {
    fn from(from: &BigInt) -> Self {
        from.0.clone()
    }
}

pub const BYTE_VAL_TRUE: u8 = 0xFF;
pub const BYTE_VAL_FALSE: u8 = 0;
pub const BYTE_VAL_SOME: u8 = 0xFF;
pub const BYTE_VAL_NONE: u8 = 0;

#[derive(PartialEq, Debug)]
pub enum Value {
    // Nothing, data is omitted from binary.
    Unit,
    /// Signed 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Int8(i8),
    /// Unsigned 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Uint8(u8),
    /// Signed 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Int16(i16),
    /// Unsigned 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Uint16(u16),
    /// Signed 31 bit integer, which corresponds to type int on 32-bit OCaml systems (data is encoded as a 32 bit int in binary and an integer in JSON).
    Int31(i32),
    /// Signed 32 bit integer (data is encoded as a 32-bit int in binary and an integer in JSON).
    Int32(i32),
    /// Signed 64 bit integer (data is encoded as a 64-bit int in binary and a decimal string in JSON).
    Int64(i64),
    /// Integer with bounds in a given range. Both bounds are inclusive.
    RangedInt(i32),
    /// Encoding of floating point number (encoded as a floating point number in JSON and a double in binary).
    Float(f64),
    /// Float with bounds in a given range. Both bounds are inclusive.
    RangedFloat(f64),
    /// Encoding of a boolean (data is encoded as a byte in binary and a boolean in JSON).
    Bool(bool),
    /// Encoding of a string
    /// - encoded as a byte sequence in binary prefixed by the length
    /// of the string
    /// - encoded as a string in JSON.
    String(String),
    /// Encoding of arbitrary bytes (encoded via hex in JSON and directly as a sequence byte in binary).
    Bytes(Vec<u8>),
    /// Combinator to make an optional value
    /// (represented as a 1-byte tag followed by the data (or nothing) in binary
    ///  and either the raw value or an empty object in JSON).
    Option(Option<Box<Value>>),
    /// Combinator to make a {!result} value
    /// (represented as a 1-byte tag followed by the data of either type in binary,
    /// and either unwrapped value in JSON (the caller must ensure that both
    /// encodings do not collide)).
    Result,
    /// Array combinator.
    /// - encoded as an array in JSON
    /// - encoded as the concatenation of all the element in binary
    /// prefixed its length in bytes
    /// If [max_length] is passed and the encoding of elements has fixed
    /// size, a {!check_size} is automatically added for earlier rejection.
    /// @raise [Invalid_argument] if the inner encoding is variable.
//    Array(Vec<Value>),
    /// List combinator.
    /// - encoded as an array in JSON
    /// - encoded as the concatenation of all the element in binary
    /// prefixed its length in bytes
    /// If [max_length] is passed and the encoding of elements has fixed
    /// size, a {!check_size} is automatically added for earlier rejection.
    /// @raise [Invalid_argument] if the inner encoding is also variable.
    List(Vec<Value>),
    // Enum value with name and/or ordinal number
    Enum(Option<String>, Option<u32>),
    // Tag value with variant id and tag inner value
    Tag(String, Box<Value>),
    /// A Record is represented by a vector of (`<record name>`, `value`).
    /// This allows schema-less encoding.
    ///
    /// See [Record](types.Record) for a more user-friendly support.
    Record(Vec<(String, Value)>),
}

