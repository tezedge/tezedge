// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::BlockHeaderWithHash;
use tezos_messages::Head;
use tezos_messages::p2p::encoding::block_header::Fitness;

use crate::state::validator::fitness_comparator::FitnessWrapper;

/// Validates if new_head is stronger or at least equals to old_head - according to fitness
pub fn can_accept_new_head(new_head: &BlockHeaderWithHash, current_head: &Head, current_context_fitness: &Fitness) -> bool {
    let new_head_fitness = FitnessWrapper::new(new_head.header.fitness());
    let current_head_fitness = FitnessWrapper::new(&current_head.fitness);
    let context_fitness = FitnessWrapper::new(current_context_fitness);

    // according to chain_validator.ml
    let accepted_head = if context_fitness.eq(&current_head_fitness) {
        new_head_fitness.gt(&current_head_fitness)
    } else {
        new_head_fitness.ge(&context_fitness)
    };

    accepted_head
}

/// Fitness comparison:
///     - shortest lists are smaller
///     - lexicographical order for lists of the same length.
pub mod fitness_comparator {
    use failure::_core::cmp::Ordering;

    use tezos_messages::p2p::encoding::block_header::Fitness;

    pub struct FitnessWrapper<'a> {
        fitness: &'a Fitness,
    }

    impl<'a> FitnessWrapper<'a> {
        pub fn new(fitness: &'a Fitness) -> Self {
            FitnessWrapper {
                fitness
            }
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
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crypto::hash::HashType;
    use tezos_messages::p2p::encoding::block_header::Fitness;
    use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

    use super::*;

    #[macro_export]
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
    fn test_can_accept_new_head() -> Result<(), failure::Error> {
        assert_eq!(false,
                   can_accept_new_head(
                       &new_head(fitness!([0]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(false,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 1]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(false,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 1]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(false,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 2]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(true,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 3]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(true,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 1], [0]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );
        assert_eq!(true,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 0, 1]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 2]),
                   )
        );

        // context fitnes is lower than current head
        assert_eq!(true,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 2]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 1]),
                   )
        );
        // context fitnes is higher than current head
        assert_eq!(false,
                   can_accept_new_head(
                       &new_head(fitness!([0], [0, 0, 2]))?,
                       &current_head(fitness!([0], [0, 0, 2]))?,
                       &fitness!([0], [0, 0, 3]),
                   )
        );

        Ok(())
    }

    fn new_head(fitness: Fitness) -> Result<BlockHeaderWithHash, failure::Error> {
        Ok(
            BlockHeaderWithHash {
                hash: HashType::BlockHash.string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
                header: Arc::new(
                    BlockHeaderBuilder::default()
                        .level(34)
                        .proto(1)
                        .predecessor(HashType::BlockHash.string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?)
                        .timestamp(5_635_634)
                        .validation_pass(4)
                        .operations_hash(HashType::OperationListListHash.string_to_bytes("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?)
                        .fitness(fitness)
                        .context(HashType::ContextHash.string_to_bytes("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?)
                        .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
                        .build().unwrap()
                ),
            }
        )
    }

    fn current_head(fitness: Fitness) -> Result<Head, failure::Error> {
        Ok(
            Head {
                hash: HashType::BlockHash.string_to_bytes("BKzyxvaMgoY5M3BUD7UaUCPivAku2NRiYRA1z1LQUzB7CX6e8yy")?,
                level: 5,
                fitness,
            }
        )
    }
}