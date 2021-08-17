// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Fitness comparison:
//!     - shortest lists are smaller
//!     - lexicographical order for lists of the same length.

use std::cmp::Ordering;

use crate::p2p::encoding::block_header::Fitness;

/// Returns only true, if right_fitness is greater than left_fitness
pub fn fitness_increases(left_fitness: &Fitness, right_fitness: &Fitness) -> bool {
    FitnessWrapper::new(right_fitness).gt(&FitnessWrapper::new(left_fitness))
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn fitness_increases_or_same(left_fitness: &Fitness, right_fitness: &Fitness) -> bool {
    FitnessWrapper::new(right_fitness).ge(&FitnessWrapper::new(left_fitness))
}

pub struct FitnessWrapper<'a> {
    fitness: &'a Fitness,
}

impl<'a> FitnessWrapper<'a> {
    pub fn new(fitness: &'a Fitness) -> Self {
        FitnessWrapper { fitness }
    }
}

impl<'a> PartialOrd for FitnessWrapper<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> Ord for FitnessWrapper<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        // length of fitness list must be equal
        let result = self.fitness.len().cmp(&other.fitness.len());
        if result != Ordering::Equal {
            return result;
        }

        // if length is same, we need to compare by elements
        let fitness_count = self.fitness.len();
        for i in 0..fitness_count {
            let self_fitness_part = &self.fitness[i];
            let other_fitness_part = &other.fitness[i];

            // length of fitness must be equal
            let result = self_fitness_part.len().cmp(&other_fitness_part.len());
            if result != Ordering::Equal {
                return result;
            }

            // now compare by-bytes from left
            let part_length = self_fitness_part.len();
            for j in 0..part_length {
                let b1 = self_fitness_part[j];
                let b2 = other_fitness_part[j];
                let byte_result = b1.cmp(&b2);
                if byte_result != Ordering::Equal {
                    return byte_result;
                }
            }
        }

        Ordering::Equal
    }
}

impl<'a> Eq for FitnessWrapper<'a> {}

impl<'a> PartialEq for FitnessWrapper<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.fitness == other.fitness
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::p2p::encoding::block_header::Fitness;

    use super::*;

    macro_rules! fitness {
        ( $($x:expr),* ) => {{
            let fitness: Fitness = vec![
                $(
                    $x.to_vec(),
                )*
            ];
            fitness
        }}
    }

    #[test]
    fn test_compare_fitness() {
        assert_eq!(
            Ordering::Equal,
            FitnessWrapper::new(&fitness!([0])).cmp(&FitnessWrapper::new(&fitness!([0])))
        );
        assert_eq!(
            Ordering::Less,
            FitnessWrapper::new(&fitness!([0])).cmp(&FitnessWrapper::new(&fitness!([1])))
        );
        assert_eq!(
            Ordering::Greater,
            FitnessWrapper::new(&fitness!([1])).cmp(&FitnessWrapper::new(&fitness!([0])))
        );

        assert_eq!(
            Ordering::Less,
            FitnessWrapper::new(&fitness!([0])).cmp(&FitnessWrapper::new(&fitness!([0], [0])))
        );
        assert_eq!(
            Ordering::Less,
            FitnessWrapper::new(&fitness!([1])).cmp(&FitnessWrapper::new(&fitness!([0], [0])))
        );

        assert_eq!(
            Ordering::Greater,
            FitnessWrapper::new(&fitness!([0], [0])).cmp(&FitnessWrapper::new(&fitness!([0])))
        );
        assert_eq!(
            Ordering::Greater,
            FitnessWrapper::new(&fitness!([0], [0])).cmp(&FitnessWrapper::new(&fitness!([1])))
        );

        assert_eq!(
            Ordering::Less,
            FitnessWrapper::new(&fitness!([0], [0])).cmp(&FitnessWrapper::new(&fitness!([0], [1])))
        );
        assert_eq!(
            Ordering::Equal,
            FitnessWrapper::new(&fitness!([0], [1])).cmp(&FitnessWrapper::new(&fitness!([0], [1])))
        );
        assert_eq!(
            Ordering::Greater,
            FitnessWrapper::new(&fitness!([0], [2])).cmp(&FitnessWrapper::new(&fitness!([0], [1])))
        );
        assert_eq!(
            Ordering::Greater,
            FitnessWrapper::new(&fitness!([2], [1])).cmp(&FitnessWrapper::new(&fitness!([2], [0])))
        );
    }

    #[test]
    fn test_fitness_increases() {
        assert!(!fitness_increases(&fitness!([0]), &fitness!([0])));
        assert!(fitness_increases_or_same(&fitness!([0]), &fitness!([0])));

        assert!(fitness_increases(&fitness!([1]), &fitness!([0], [0])));
        assert!(fitness_increases_or_same(
            &fitness!([1]),
            &fitness!([0], [0])
        ));

        assert!(fitness_increases(&fitness!([0]), &fitness!([1])));
        assert!(fitness_increases_or_same(&fitness!([0]), &fitness!([1])));

        assert!(fitness_increases(&fitness!([0]), &fitness!([0], [0])));
        assert!(fitness_increases_or_same(
            &fitness!([0]),
            &fitness!([0], [0])
        ));

        assert!(!fitness_increases(&fitness!([0], [0]), &fitness!([0], [0])));
        assert!(fitness_increases_or_same(
            &fitness!([0], [0]),
            &fitness!([0], [0])
        ));

        assert!(fitness_increases(&fitness!([0], [0]), &fitness!([0], [1])));
        assert!(fitness_increases_or_same(
            &fitness!([0], [0]),
            &fitness!([0], [1])
        ));

        assert!(!fitness_increases(&fitness!([1], [0]), &fitness!([0], [1])));
        assert!(!fitness_increases_or_same(
            &fitness!([1], [0]),
            &fitness!([0], [1])
        ));

        assert!(fitness_increases(
            &fitness!([0], [0, 0, 2]),
            &fitness!([0], [0, 0, 3])
        ));
        assert!(fitness_increases_or_same(
            &fitness!([0], [0, 0, 2]),
            &fitness!([0], [0, 0, 3])
        ));
    }
}
