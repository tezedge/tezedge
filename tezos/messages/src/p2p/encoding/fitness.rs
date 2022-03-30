// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;

use tezos_encoding::{
    enc::BinWriter,
    encoding::{Encoding, HasEncoding},
    nom::NomReader,
};

use super::limits::BLOCK_HEADER_FITNESS_MAX_SIZE;

pub type FitnessInner = Vec<Vec<u8>>;

pub struct FitnessRef<'a>(pub &'a FitnessInner);

impl<'a> FitnessRef<'a> {
    pub fn as_hex_vec(&self) -> Vec<String> {
        self.0.iter().map(hex::encode).collect()
    }
}

impl<'a> std::fmt::Display for FitnessRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut i = self.0.iter();
        if let Some(first) = i.next() {
            write!(f, "{first}", first = hex::encode(first))?;
            for next in i {
                write!(f, "::{next}", next = hex::encode(next))?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Fitness(FitnessInner);

impl Fitness {
    pub fn from_bytes<T, I>(bytes: T) -> Self
    where
        T: IntoIterator<Item = I>,
        I: AsRef<[u8]>,
    {
        Self(bytes.into_iter().map(|i| i.as_ref().to_vec()).collect())
    }

    pub fn to_inner(self) -> FitnessInner {
        self.0
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn as_hex_vec(&self) -> Vec<String> {
        FitnessRef(&self.0).as_hex_vec()
    }

    pub fn round(&self) -> Option<i32> {
        self.0.get(4).and_then(|bytes| {
            if bytes.len() != 4 {
                return None;
            }
            Some(i32::from_be_bytes(bytes[0..4].try_into().unwrap()))
        })
    }
}

impl AsRef<FitnessInner> for Fitness {
    fn as_ref(&self) -> &FitnessInner {
        &self.0
    }
}

impl Ord for Fitness {
    fn cmp(&self, other: &Self) -> Ordering {
        let len_ord = self.0.len().cmp(&other.0.len());
        if len_ord != Ordering::Equal {
            return len_ord;
        }
        for (s, o) in self.0.iter().zip(other.0.iter()) {
            let item_len_ord = s.len().cmp(&o.len());
            if item_len_ord != Ordering::Equal {
                return item_len_ord;
            }
            for (sb, ob) in s.iter().zip(o.iter()) {
                let byte_ord = sb.cmp(ob);
                if byte_ord != Ordering::Equal {
                    return byte_ord;
                }
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for Fitness {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl NomReader for Fitness {
    fn nom_read(input: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        use tezos_encoding::nom::*;
        let (input, bytes) =
            bounded_dynamic(BLOCK_HEADER_FITNESS_MAX_SIZE, list(dynamic(bytes)))(input)?;
        Ok((input, Self(bytes)))
    }
}

impl BinWriter for Fitness {
    fn bin_write(&self, out: &mut Vec<u8>) -> tezos_encoding::enc::BinResult {
        use tezos_encoding::enc::*;
        bounded_dynamic(BLOCK_HEADER_FITNESS_MAX_SIZE, list(dynamic(bytes)))(&self.0, out)
    }
}

impl HasEncoding for Fitness {
    fn encoding() -> tezos_encoding::encoding::Encoding {
        Encoding::bounded_dynamic(
            BLOCK_HEADER_FITNESS_MAX_SIZE,
            Encoding::list(Encoding::dynamic(Encoding::list(Encoding::Uint8))),
        )
    }
}

impl serde::Serialize for Fitness {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            self.0
                .iter()
                .map(hex::encode)
                .collect::<Vec<_>>()
                .serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Fitness {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let vec_of_strings: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
            let vec_of_bytes = vec_of_strings
                .into_iter()
                .map(hex::decode)
                .collect::<Result<_, _>>()
                .map_err(|e| {
                    serde::de::Error::custom(format!("failed to convert hex string to bytes: {e}"))
                })?;
            Ok(Fitness(vec_of_bytes))
        } else {
            serde::Deserialize::deserialize(deserializer).map(Fitness)
        }
    }
}

impl From<Vec<Vec<u8>>> for Fitness {
    fn from(source: Vec<Vec<u8>>) -> Self {
        Self(source)
    }
}

impl std::fmt::Debug for Fitness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "fitness: {self}")
    }
}

impl std::fmt::Display for Fitness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        FitnessRef(&self.0).fmt(f)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    macro_rules! fitness {
        ( $($x:expr),* ) => {{
            Fitness::from(vec![
                $(
                    $x.to_vec(),
                )*
            ])
        }}
    }

    #[test]
    fn fitness_json() {
        let json_fitness = serde_json::json!(["01", "0000000a"]);
        let fitness = Fitness::from_bytes(&[&[1_u8][..], &0x0a_i32.to_be_bytes()[..]]);
        assert_eq!(
            serde_json::to_string(&json_fitness).unwrap(),
            serde_json::to_string(&fitness).unwrap()
        );
    }

    #[test]
    fn fitness_ord() {
        assert_eq!(fitness!([0]).cmp(&fitness!([0])), Ordering::Equal);
        assert_eq!(fitness!([1]).cmp(&fitness!([0])), Ordering::Greater);
        assert_eq!(fitness!([1]).cmp(&fitness!([0, 0])), Ordering::Less);
        assert_eq!(fitness!([0, 1]).cmp(&fitness!([0, 0])), Ordering::Greater);
        assert_eq!(
            fitness!([0], [0, 1]).cmp(&fitness!([0], [0, 0, 2])),
            Ordering::Less
        );
    }
}
