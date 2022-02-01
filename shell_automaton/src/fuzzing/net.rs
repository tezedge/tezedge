use fuzzcheck::fastrand;
use fuzzcheck::Mutator;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;

use crate::peer::PeerToken;

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
                *step = ArbitraryStep::Once;
                let state = FUZZER_STATE.read().unwrap();
                let addresses: Vec<&SocketAddr> =
                    state.current_target_state.peers.iter_addr().collect();
                let i = self.rng.usize(..addresses.len() + 1);

                /*
                    This serves 2 purposes:
                    1 - Fallback for empty addresses list.
                    2 - Randomly generate addresses with a probability inversely
                        proportional to the length of the list.
                */
                let addr = if i == addresses.len() {
                    SocketAddr::new(
                        IpAddr::V4(Ipv4Addr::new(
                            self.rng.u8(..u8::MAX),
                            self.rng.u8(..u8::MAX),
                            self.rng.u8(..u8::MAX),
                            self.rng.u8(..u8::MAX),
                        )),
                        self.rng.u16(..u16::MAX),
                    )
                } else {
                    *addresses[i]
                };

                Some((addr, SOCKADDR_COMPLEXITY))
            }
            ArbitraryStep::Once => None,
        }
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_arbitrary(&self, _max_cplx: f64) -> (SocketAddr, f64) {
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.current_target_state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len() + 1);

        let addr = if i == addresses.len() {
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                )),
                self.rng.u16(..u16::MAX),
            )
        } else {
            *addresses[i]
        };

        (addr, SOCKADDR_COMPLEXITY)
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
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.current_target_state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len() + 1);

        let addr = if i == addresses.len() {
            SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                    self.rng.u8(..u8::MAX),
                )),
                self.rng.u16(..u16::MAX),
            )
        } else {
            *addresses[i]
        };

        (std::mem::replace(value, addr), SOCKADDR_COMPLEXITY)
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

#[derive(Default)]
pub struct IpAddrMutator {
    rng: fastrand::Rng,
}

const IPADDR_COMPLEXITY: f64 = 1.0;

impl Mutator<IpAddr> for IpAddrMutator {
    #[doc(hidden)]
    type Cache = ();
    #[doc(hidden)]
    type MutationStep = bool;
    #[doc(hidden)]
    type ArbitraryStep = ArbitraryStep;
    #[doc(hidden)]
    type UnmutateToken = IpAddr;

    #[doc(hidden)]
    #[no_coverage]
    fn default_arbitrary_step(&self) -> Self::ArbitraryStep {
        <_>::default()
    }

    #[doc(hidden)]
    #[no_coverage]
    fn max_complexity(&self) -> f64 {
        IPADDR_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn min_complexity(&self) -> f64 {
        IPADDR_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn validate_value(&self, _value: &IpAddr) -> Option<(Self::Cache, Self::MutationStep)> {
        Some(((), INITIAL_MUTATION_STEP))
    }
    #[doc(hidden)]
    #[no_coverage]
    fn complexity(&self, _value: &IpAddr, _cache: &Self::Cache) -> f64 {
        IPADDR_COMPLEXITY
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_arbitrary(
        &self,
        step: &mut Self::ArbitraryStep,
        max_cplx: f64,
    ) -> Option<(IpAddr, f64)> {
        if max_cplx < self.min_complexity() {
            return None;
        }
        match step {
            ArbitraryStep::Never => {
                *step = ArbitraryStep::Once;
                let state = FUZZER_STATE.read().unwrap();
                let addresses: Vec<&SocketAddr> =
                    state.current_target_state.peers.iter_addr().collect();
                let i = self.rng.usize(..addresses.len() + 1);

                let addr = if i == addresses.len() {
                    IpAddr::V4(Ipv4Addr::new(
                        self.rng.u8(..u8::MAX),
                        self.rng.u8(..u8::MAX),
                        self.rng.u8(..u8::MAX),
                        self.rng.u8(..u8::MAX),
                    ))
                } else {
                    addresses[i].ip()
                };

                Some((addr, IPADDR_COMPLEXITY))
            }
            ArbitraryStep::Once => None,
        }
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_arbitrary(&self, _max_cplx: f64) -> (IpAddr, f64) {
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.current_target_state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len() + 1);
        let addr = if i == addresses.len() {
            IpAddr::V4(Ipv4Addr::new(
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
            ))
        } else {
            addresses[i].ip()
        };

        (addr, IPADDR_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_mutate(
        &self,
        _value: &mut IpAddr,
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
        value: &mut IpAddr,
        _cache: &mut Self::Cache,
        _max_cplx: f64,
    ) -> (Self::UnmutateToken, f64) {
        let state = FUZZER_STATE.read().unwrap();
        let addresses: Vec<&SocketAddr> = state.current_target_state.peers.iter_addr().collect();
        let i = self.rng.usize(..addresses.len() + 1);

        let addr = if i == addresses.len() {
            IpAddr::V4(Ipv4Addr::new(
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
                self.rng.u8(..u8::MAX),
            ))
        } else {
            addresses[i].ip()
        };

        (std::mem::replace(value, addr), IPADDR_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn unmutate(&self, value: &mut IpAddr, _cache: &mut Self::Cache, t: Self::UnmutateToken) {
        *value = t;
    }

    #[doc(hidden)]
    type RecursingPartIndex = ();
    #[doc(hidden)]
    #[no_coverage]
    fn default_recursing_part_index(
        &self,
        _value: &IpAddr,
        _cache: &Self::Cache,
    ) -> Self::RecursingPartIndex {
    }
    #[doc(hidden)]
    #[no_coverage]
    fn recursing_part<'a, T, M>(
        &self,
        _parent: &M,
        _value: &'a IpAddr,
        _index: &mut Self::RecursingPartIndex,
    ) -> Option<&'a T>
    where
        T: Clone,
        M: Mutator<T>,
    {
        None
    }
}

#[derive(Default)]
pub struct PeerTokenMutator {
    rng: fastrand::Rng,
}

const TOKEN_COMPLEXITY: f64 = 1.0;

impl Mutator<PeerToken> for PeerTokenMutator {
    #[doc(hidden)]
    type Cache = ();
    #[doc(hidden)]
    type MutationStep = bool;
    #[doc(hidden)]
    type ArbitraryStep = ArbitraryStep;
    #[doc(hidden)]
    type UnmutateToken = PeerToken;

    #[doc(hidden)]
    #[no_coverage]
    fn default_arbitrary_step(&self) -> Self::ArbitraryStep {
        <_>::default()
    }

    #[doc(hidden)]
    #[no_coverage]
    fn max_complexity(&self) -> f64 {
        TOKEN_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn min_complexity(&self) -> f64 {
        TOKEN_COMPLEXITY
    }

    #[doc(hidden)]
    #[no_coverage]
    fn validate_value(&self, _value: &PeerToken) -> Option<(Self::Cache, Self::MutationStep)> {
        Some(((), INITIAL_MUTATION_STEP))
    }
    #[doc(hidden)]
    #[no_coverage]
    fn complexity(&self, _value: &PeerToken, _cache: &Self::Cache) -> f64 {
        TOKEN_COMPLEXITY
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_arbitrary(
        &self,
        step: &mut Self::ArbitraryStep,
        max_cplx: f64,
    ) -> Option<(PeerToken, f64)> {
        if max_cplx < self.min_complexity() {
            return None;
        }
        match step {
            ArbitraryStep::Never => {
                *step = ArbitraryStep::Once;
                let state = FUZZER_STATE.read().unwrap();
                let tokens: Vec<PeerToken> = state
                    .current_target_state
                    .peers
                    .iter()
                    .filter_map(|(addr, peer)| peer.token())
                    .collect();

                let i = self.rng.usize(..tokens.len() + 1);

                let token = if i == tokens.len() {
                    PeerToken::new_unchecked(self.rng.usize(..usize::MAX))
                } else {
                    tokens[i]
                };

                Some((token, TOKEN_COMPLEXITY))
            }
            ArbitraryStep::Once => None,
        }
    }
    #[doc(hidden)]
    #[no_coverage]
    fn random_arbitrary(&self, _max_cplx: f64) -> (PeerToken, f64) {
        let state = FUZZER_STATE.read().unwrap();
        let tokens: Vec<PeerToken> = state
            .current_target_state
            .peers
            .iter()
            .filter_map(|(addr, peer)| peer.token())
            .collect();

        let i = self.rng.usize(..tokens.len() + 1);

        let token = if i == tokens.len() {
            PeerToken::new_unchecked(self.rng.usize(..usize::MAX))
        } else {
            tokens[i]
        };

        (token, TOKEN_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn ordered_mutate(
        &self,
        _value: &mut PeerToken,
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
        value: &mut PeerToken,
        _cache: &mut Self::Cache,
        _max_cplx: f64,
    ) -> (Self::UnmutateToken, f64) {
        let state = FUZZER_STATE.read().unwrap();
        let tokens: Vec<PeerToken> = state
            .current_target_state
            .peers
            .iter()
            .filter_map(|(addr, peer)| peer.token())
            .collect();

        let i = self.rng.usize(..tokens.len() + 1);

        let token = if i == tokens.len() {
            PeerToken::new_unchecked(self.rng.usize(..usize::MAX))
        } else {
            tokens[i]
        };

        (std::mem::replace(value, token), TOKEN_COMPLEXITY)
    }
    #[doc(hidden)]
    #[no_coverage]
    fn unmutate(&self, value: &mut PeerToken, _cache: &mut Self::Cache, t: Self::UnmutateToken) {
        *value = t;
    }

    #[doc(hidden)]
    type RecursingPartIndex = ();
    #[doc(hidden)]
    #[no_coverage]
    fn default_recursing_part_index(
        &self,
        _value: &PeerToken,
        _cache: &Self::Cache,
    ) -> Self::RecursingPartIndex {
    }
    #[doc(hidden)]
    #[no_coverage]
    fn recursing_part<'a, T, M>(
        &self,
        _parent: &M,
        _value: &'a PeerToken,
        _index: &mut Self::RecursingPartIndex,
    ) -> Option<&'a T>
    where
        T: Clone,
        M: Mutator<T>,
    {
        None
    }
}
