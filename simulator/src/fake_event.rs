use std::io;
use std::time::Instant;

use tezedge_state::proposer::{Event, EventRef, NetworkEvent};

use crate::fake_peer_id::FakePeerId;

pub type FakeEvent = Event<FakeNetworkEvent>;
pub type FakeEventRef<'a> = EventRef<'a, FakeNetworkEvent>;

#[derive(Debug, Clone)]
pub enum FakeNetworkEventType {
    IncomingConnection,
    Disconnected,

    BytesWritable(Option<usize>),
    BytesReadable(Option<usize>),
    BytesWritableError(io::ErrorKind),
    BytesReadableError(io::ErrorKind),
}

#[derive(Debug, Clone)]
pub struct FakeNetworkEvent {
    pub time: Instant,
    pub from: FakePeerId,
    pub event_type: FakeNetworkEventType,
}

impl NetworkEvent for FakeNetworkEvent {
    fn is_server_event(&self) -> bool {
        matches!(&self.event_type, FakeNetworkEventType::IncomingConnection)
    }

    fn is_readable(&self) -> bool {
        use FakeNetworkEventType::*;
        matches!(&self.event_type, BytesReadable(_) | BytesReadableError(_))
    }

    fn is_writable(&self) -> bool {
        use FakeNetworkEventType::*;
        matches!(&self.event_type, BytesWritable(_) | BytesWritableError(_))
    }

    fn is_read_closed(&self) -> bool {
        matches!(&self.event_type, FakeNetworkEventType::Disconnected)
    }

    fn is_write_closed(&self) -> bool {
        false
    }

    fn time(&self) -> Instant {
        self.time
    }
}
