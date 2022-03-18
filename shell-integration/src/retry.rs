// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

/// RetryPolicy could be used for some handling, where we want to "retry some operation on <subject>" for number of available retries
#[derive(Clone, Debug)]
pub struct RetryPolicy<T: Clone + PartialEq + Eq> {
    subject: T,
    retries: u8,
    max_retries: u8,
}

impl<T: Clone + PartialEq + Eq> RetryPolicy<T> {
    pub fn new(subject: T, retries: u8) -> Self {
        Self {
            subject,
            max_retries: retries,
            retries,
        }
    }

    pub fn next_retry(mut self, requested_subject: &T) -> Option<RetryPolicy<T>> {
        if self.subject.eq(requested_subject) {
            if self.retries > 0 {
                // remove one try and continue
                self.retries = self.retries.saturating_sub(1);
                Some(self)
            } else {
                // stop, no more retries
                None
            }
        } else {
            // if requested block is different as last retry_block, we create new policy,
            // means, the last retry_block passed so we are handling next block
            Some(RetryPolicy::new(
                requested_subject.clone(),
                self.max_retries,
            ))
        }
    }
}

#[cfg(test)]
pub mod tests {
    use std::convert::TryInto;
    use std::sync::Arc;

    use crypto::hash::BlockHash;

    use super::*;

    fn block(b: u8) -> Arc<BlockHash> {
        Arc::new(
            [b; crypto::hash::HashType::BlockHash.size()]
                .to_vec()
                .try_into()
                .expect("Failed to create BlockHash"),
        )
    }

    #[test]
    fn test_retry_policy_for_the_same_block() {
        let block_5 = block(5);
        let retry_policy = RetryPolicy::new(block_5.clone(), 3);

        // first retry
        let retry_policy = retry_policy.next_retry(&block_5).unwrap();
        assert_eq!(2, retry_policy.retries);

        // second retry
        let retry_policy = retry_policy.next_retry(&block_5).unwrap();
        assert_eq!(1, retry_policy.retries);

        // third retry
        let retry_policy = retry_policy.next_retry(&block_5).unwrap();
        assert_eq!(0, retry_policy.retries);

        // fourth retry
        assert!(retry_policy.next_retry(&block_5).is_none());
    }

    #[test]
    fn test_retry_policy_for_the_different_block() {
        let block_5 = block(5);
        let block_6 = block(6);
        let retry_policy = RetryPolicy::new(block_5.clone(), 3);

        // first retry for 5
        let retry_policy = retry_policy.next_retry(&block_5).unwrap();
        assert_eq!(2, retry_policy.retries);

        // second retry for 5
        let retry_policy = retry_policy.next_retry(&block_5).unwrap();
        assert_eq!(1, retry_policy.retries);

        // third retry for 6 - creates new retry policy
        let retry_policy = retry_policy.next_retry(&block_6).unwrap();
        assert_eq!(3, retry_policy.retries);
    }

    #[test]
    fn test_retry_policy_zero_retries() {
        let block_5 = block(5);
        let retry_policy = RetryPolicy::new(block_5.clone(), 0);

        // first retry
        assert!(retry_policy.next_retry(&block_5).is_none());
    }
}
