use std::time::Instant;

/// Proposal is the message directed towards state machine.
///
/// By sending proposals, state can be mutated/updated.
///
/// Each proposal updates logical clock of the state machine, after
/// it has been fed to `Acceptor`.
pub trait Proposal {
    fn time(&self) -> Instant;
}
