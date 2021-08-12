/// Acceptor is what receives a proposal and handles it. Might mutate
/// state based on proposal.
///
/// Acceptor is the only source of input for state machine. Every
/// command/message/event must be supplied through the acceptor. State
/// can be accessed directly using other methods, but the only way
/// to mutate state should be through acceptor.
///
/// Anything implementing an `Acceptor` must be determenistic, in a sense
/// that if we replay same set of proposals countless times, we should
/// get the exact same resulting state.
pub trait Acceptor<P> {
    fn accept(&mut self, proposal: P);
}
