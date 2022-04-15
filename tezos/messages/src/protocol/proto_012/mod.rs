// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::p2p::encoding::fitness::{Fitness, FitnessRef};

pub mod block_header;
pub mod constants;
pub mod contract;
pub mod operation;
pub mod rights;
pub mod votes;

pub const PROTOCOL_HASH: &str = "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A";

pub struct FitnessRepr {
    pub level: i32,
    pub locked_round: Option<i32>,
    pub predecessor_round: i32,
    pub round: i32,
}

#[derive(Debug, thiserror::Error)]
#[error("Incorrect fitness version")]
pub struct IncorrectFitness;

fn slice_to_i32(s: &[u8]) -> Result<i32, IncorrectFitness> {
    Ok(i32::from_be_bytes(
        s.try_into().map_err(|_| IncorrectFitness)?,
    ))
}

const VERSION: &[u8] = &[0x02];

impl<'a> TryFrom<FitnessRef<'a>> for FitnessRepr {
    type Error = IncorrectFitness;

    fn try_from(value: FitnessRef) -> Result<Self, Self::Error> {
        let slice = value.0.as_slice();
        if slice.len() != 5 {
            return Err(IncorrectFitness);
        }
        if &slice[0] != VERSION {
            return Err(IncorrectFitness);
        }
        let level = slice_to_i32(&slice[1])?;
        let locked_round = if slice[2].is_empty() {
            None
        } else {
            Some(slice_to_i32(&slice[2])?)
        };
        let predecessor_round = -slice_to_i32(&slice[3])? - 1;
        let round = slice_to_i32(&slice[4])?;
        Ok(Self {
            level,
            locked_round,
            predecessor_round,
            round,
        })
    }
}

impl TryFrom<&Fitness> for FitnessRepr {
    type Error = IncorrectFitness;

    fn try_from(value: &Fitness) -> Result<Self, Self::Error> {
        FitnessRef(value.as_ref()).try_into()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::p2p::encoding::fitness::Fitness;

    use super::FitnessRepr;

    #[test]
    fn fitness_repr() {
        let fitness = Fitness::from_str("02::000631a5::::ffffffff::00000000").unwrap();
        let fitness_repr = FitnessRepr::try_from(&fitness).unwrap();
        assert_eq!(fitness_repr.level, 0x000631a5);
        assert_eq!(fitness_repr.locked_round, None);
        assert_eq!(fitness_repr.predecessor_round, 0);
        assert_eq!(fitness_repr.round, 0);

        let fitness = Fitness::from_str("02::000631a5::00000001::ffffffff::00000002").unwrap();
        let fitness_repr = FitnessRepr::try_from(&fitness).unwrap();
        assert_eq!(fitness_repr.level, 0x000631a5);
        assert_eq!(fitness_repr.locked_round, Some(1));
        assert_eq!(fitness_repr.predecessor_round, 0);
        assert_eq!(fitness_repr.round, 2);
    }
}
