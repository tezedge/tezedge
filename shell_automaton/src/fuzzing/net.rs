use fuzzcheck::fastrand;
use fuzzcheck::Mutator;
use std::net::SocketAddr;

use super::state_singleton::FUZZER_STATE;

#[derive(Default)]
pub struct SocketAddrMutator {
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

const SOCKADDR_COMPLEXITY: f64 = 1.0;
const INITIAL_MUTATION_STEP: bool = false;

impl Mutator<SocketAddr> for SocketAddrMutator {
    #[doc(hidden)]
    type Cache = ();
    #[doc(hidden)]
    type MutationStep = bool;
    #[doc(hidden)]
    type ArbitraryStep = ArbitraryStep;
    #[doc(hidden)]
    type UnmutateToken = SocketAddr;

    #[doc(hidden)]
    #[no_coverage]
    fn default_arbitrary_step(&self) -> Self::ArbitraryStep {
        <_>::default()
    }

    #[doc(hidden)]
    #[no_coverage]
    fn max_complexity(&self) -> f64 {
        SOCKADDR_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn min_complexity(&self) -> f64 {
        SOCKADDR_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn validate_value(&self, _value: &SocketAddr) -> Option<(Self::Cache, Self::MutationStep)> {
        Some(((), INITIAL_MUTATION_STEP))
    }
    #[doc(hidden)]
    #[no_coverage]
    fn complexity(&self, _value: &SocketAddr, _cache: &Self::Cache) -> f64 {
        SOCKADDR_COMPLEXITY
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_arbitrary(
        &self,
        step: &mut Self::ArbitraryStep,
        max_cplx: f64,
    ) -> Option<(SocketAddr, f64)> {
        if max_cplx < self.min_complexity() {
            return None;
        }
        match step {
            ArbitraryStep::Never => {
                // TODO: handle empty range (no connected peers)
                *step = ArbitraryStep::Once;
                let state = FUZZER_STATE.read().unwrap();
                let addresses: Vec<&SocketAddr> = state.peers.iter_addr().collect();
                let i = self.rng.usize(..addresses.len());
                Some((*addresses[i], SOCKADDR_COMPLEXITY))
            }
            ArbitraryStep::Once => None,
        }
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_arbitrary(&self, _max_cplx: f64) -> (SocketAddr, f64) {
        // TODO: handle empty range (no connected peers)
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len());

        (*addresses[i], SOCKADDR_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_mutate(
        &self,
        _value: &mut SocketAddr,
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
        value: &mut SocketAddr,
        _cache: &mut Self::Cache,
        _max_cplx: f64,
    ) -> (Self::UnmutateToken, f64) {
        // TODO: handle empty range (no connected peers)
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len());

        (std::mem::replace(value, *addresses[i]), SOCKADDR_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn unmutate(&self, value: &mut SocketAddr, _cache: &mut Self::Cache, t: Self::UnmutateToken) {
        *value = t;
    }

    #[doc(hidden)]
    type RecursingPartIndex = ();
    #[doc(hidden)]
    #[no_coverage]
    fn default_recursing_part_index(
        &self,
        _value: &SocketAddr,
        _cache: &Self::Cache,
    ) -> Self::RecursingPartIndex {
    }
    #[doc(hidden)]
    #[no_coverage]
    fn recursing_part<'a, T, M>(
        &self,
        _parent: &M,
        _value: &'a SocketAddr,
        _index: &mut Self::RecursingPartIndex,
    ) -> Option<&'a T>
    where
        T: Clone,
        M: Mutator<T>,
    {
        None
    }
}
