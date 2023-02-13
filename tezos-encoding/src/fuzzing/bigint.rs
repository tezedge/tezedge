// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use fuzzcheck::fastrand;
use fuzzcheck::Mutator;
use num_bigint::BigInt;

#[derive(Default)]
pub struct BigIntMutator {
    rng: fastrand::Rng,
}

#[doc(hidden)]
#[derive(Clone, Copy)]
pub enum ArbitraryStep {
    Never = 0,
    Once = 1,
}
impl Default for ArbitraryStep {
    #[no_coverage]
    fn default() -> Self {
        Self::Never
    }
}

const BIGINT_COMPLEXITY: f64 = 1.0;
const INITIAL_MUTATION_STEP: bool = false;

impl Mutator<BigInt> for BigIntMutator {
    #[doc(hidden)]
    type Cache = ();
    #[doc(hidden)]
    type MutationStep = bool;
    #[doc(hidden)]
    type ArbitraryStep = ArbitraryStep;
    #[doc(hidden)]
    type UnmutateToken = BigInt;

    #[doc(hidden)]
    #[no_coverage]
    fn default_arbitrary_step(&self) -> Self::ArbitraryStep {
        <_>::default()
    }

    #[doc(hidden)]
    #[no_coverage]
    fn max_complexity(&self) -> f64 {
        BIGINT_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn min_complexity(&self) -> f64 {
        BIGINT_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn validate_value(&self, _value: &BigInt) -> Option<()> {
        Some(())
    }

    #[doc(hidden)]
    #[no_coverage]
    fn default_mutation_step(&self, _value: &BigInt, _cache: &Self::Cache) -> Self::MutationStep {
        INITIAL_MUTATION_STEP
    }

    #[doc(hidden)]
    #[no_coverage]
    fn complexity(&self, _value: &BigInt, _cache: &Self::Cache) -> f64 {
        BIGINT_COMPLEXITY
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_arbitrary(
        &self,
        step: &mut Self::ArbitraryStep,
        max_cplx: f64,
    ) -> Option<(BigInt, f64)> {
        if max_cplx < self.min_complexity() {
            return None;
        }
        match step {
            ArbitraryStep::Never => {
                *step = ArbitraryStep::Once;
                Some((BigInt::from(self.rng.u64(..u64::MAX)), BIGINT_COMPLEXITY))
            }
            ArbitraryStep::Once => None,
        }
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_arbitrary(&self, _max_cplx: f64) -> (BigInt, f64) {
        (BigInt::from(self.rng.u64(..u64::MAX)), BIGINT_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_mutate(
        &self,
        _value: &mut BigInt,
        _cache: &mut Self::Cache,
        _step: &mut Self::MutationStep,
        _max_cplx: f64,
    ) -> Option<(Self::UnmutateToken, f64)> {
        None
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_mutate(
        &self,
        value: &mut BigInt,
        _cache: &mut Self::Cache,
        _max_cplx: f64,
    ) -> (Self::UnmutateToken, f64) {
        (
            std::mem::replace(value, BigInt::from(self.rng.u64(..u64::MAX))),
            BIGINT_COMPLEXITY,
        )
    }
    #[doc(hidden)]
    #[no_coverage]
    fn unmutate(&self, value: &mut BigInt, _cache: &mut Self::Cache, t: Self::UnmutateToken) {
        *value = t;
    }

    #[doc(hidden)]
    type RecursingPartIndex = ();
    #[doc(hidden)]
    #[no_coverage]
    fn default_recursing_part_index(
        &self,
        _value: &BigInt,
        _cache: &Self::Cache,
    ) -> Self::RecursingPartIndex {
    }
    #[doc(hidden)]
    #[no_coverage]
    fn recursing_part<'a, T, M>(
        &self,
        _parent: &M,
        _value: &'a BigInt,
        _index: &mut Self::RecursingPartIndex,
    ) -> Option<&'a T>
    where
        T: Clone,
        M: Mutator<T>,
    {
        None
    }
}
