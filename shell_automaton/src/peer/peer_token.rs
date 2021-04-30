use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
pub struct PeerToken(usize);

impl PeerToken {
    /// Caller must make sure token is correct and is associated with
    /// some connected peer.
    #[inline(always)]
    pub fn new_unchecked(token: usize) -> Self {
        Self(token)
    }

    #[inline(always)]
    pub fn index(&self) -> usize {
        self.0
    }
}

impl From<PeerToken> for usize {
    fn from(val: PeerToken) -> usize {
        val.0
    }
}

impl From<PeerToken> for mio::Token {
    fn from(val: PeerToken) -> mio::Token {
        mio::Token(val.0)
    }
}
