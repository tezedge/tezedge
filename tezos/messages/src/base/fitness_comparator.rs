// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Fitness comparison:
//!     - shortest lists are smaller
//!     - lexicographical order for lists of the same length.

use crate::p2p::encoding::fitness::Fitness;

/// Returns only true, if right_fitness is greater than left_fitness
pub fn fitness_increases(left_fitness: &Fitness, right_fitness: &Fitness) -> bool {
    left_fitness < right_fitness
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn fitness_increases_or_same(left_fitness: &Fitness, right_fitness: &Fitness) -> bool {
    left_fitness <= right_fitness
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::p2p::encoding::fitness::Fitness;

    use super::*;

    macro_rules! fitness {
        ( $($x:expr),* ) => {{
            let fitness: Fitness = vec![
                $(
                    $x.to_vec(),
                )*
            ].into();
            fitness
        }}
    }

    #[test]
    fn test_compare_fitness() {
        assert_eq!(Ordering::Equal, fitness!([0]).cmp(&fitness!([0])));
        assert_eq!(Ordering::Less, fitness!([0]).cmp(&fitness!([1])));
        assert_eq!(Ordering::Greater, fitness!([1]).cmp(&fitness!([0])));

        assert_eq!(Ordering::Less, fitness!([0]).cmp(&fitness!([0], [0])));
        assert_eq!(Ordering::Less, fitness!([1]).cmp(&fitness!([0], [0])));

        assert_eq!(Ordering::Greater, fitness!([0], [0]).cmp(&fitness!([0])));
        assert_eq!(Ordering::Greater, fitness!([0], [0]).cmp(&fitness!([1])));

        assert_eq!(Ordering::Less, fitness!([0], [0]).cmp(&fitness!([0], [1])));
        assert_eq!(Ordering::Equal, fitness!([0], [1]).cmp(&fitness!([0], [1])));
        assert_eq!(
            Ordering::Greater,
            fitness!([0], [2]).cmp(&fitness!([0], [1]))
        );
        assert_eq!(
            Ordering::Greater,
            fitness!([2], [1]).cmp(&fitness!([2], [0]))
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
