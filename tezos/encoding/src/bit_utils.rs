use bitvec::{BitVec, Cursor, Bits};

pub trait BitReverse {
    fn reverse(&self) -> Self;
}

impl <E,T> BitReverse for BitVec<E,T>
    where
        E: Cursor,
        T: Bits
{
    #[inline]
    fn reverse(&self) -> BitVec<E,T> {
        let mut reversed: BitVec<E,T> = BitVec::new();
        for bit in self.iter().rev() {
            reversed.push(bit)
        }
        reversed
    }
}


pub trait BitTrim {
    fn trim_left(&self) -> Self;
}

impl <E,T> BitTrim for BitVec<E,T>
    where
        E: Cursor,
        T: Bits
{
    fn trim_left(&self) -> BitVec<E,T> {
        let mut trimmed: BitVec<E,T> = BitVec::new();

        let mut notrim = false;
        for bit in self.iter() {
            if bit {
                trimmed.push(bit);
                notrim = true;
            } else if notrim {
                trimmed.push(bit);
            }
        }
        trimmed
    }
}

pub trait ToBytes {
    fn to_byte_vec(&self) -> Vec<u8>;
}

impl <E,T> ToBytes for BitVec<E,T>
    where
        E: Cursor,
        T: Bits
{
    fn to_byte_vec(&self) -> Vec<u8> {
        let mut bytes = vec![];
        let mut byte = 0;
        let mut offset = 0;
        for (idx_bit, bit) in self.iter().rev().enumerate() {
            let idx_byte = (idx_bit % 8) as u8;
            byte.set(idx_byte, bit);
            if idx_byte == 7 {
                bytes.push(byte);
                byte = 0;
            }
            offset = idx_byte;
        }
        if offset != 7 {
            bytes.push(byte);
        }
        bytes.reverse();
        bytes
    }
}
