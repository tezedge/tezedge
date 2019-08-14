pub type ChainId = Vec<u8>;
pub type BlockHash = Vec<u8>;

pub mod operation;
pub mod version;
pub mod connection;
pub mod metadata;
pub mod ack;
pub mod get_current_branch;
pub mod current_branch;
pub mod get_current_head;
pub mod current_head;
pub mod mempool;
pub mod block_header;
pub mod peer;

pub mod prelude {
    pub use super::connection::ConnectionMessage;
    pub use super::metadata::MetadataMessage;
    pub use super::ack::AckMessage;
    pub use super::get_current_branch::GetCurrentBranchMessage;
    pub use super::current_branch::CurrentBranchMessage;
    pub use super::get_current_head::GetCurrentHeadMessage;
    pub use super::current_head::CurrentHeadMessage;
    pub use super::peer::PeerMessage;
    pub use super::peer::PeerMessageResponse;
}