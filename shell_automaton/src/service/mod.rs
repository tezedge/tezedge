// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use redux_rs::TimeService;

pub mod service_channel;

pub mod dns_service;
pub use dns_service::{DnsService, DnsServiceDefault};

pub mod randomness_service;
pub use randomness_service::{RandomnessService, RandomnessServiceDefault};

pub mod mio_service;
pub use mio_service::{MioService, MioServiceDefault};

pub mod storage_service;
pub use storage_service::{StorageService, StorageServiceDefault};

pub mod rpc_service;
pub use rpc_service::{RpcService, RpcServiceDefault};

pub mod actors_service;
pub use actors_service::{ActorsService, ActorsServiceDefault};

mod quota_service;
pub use quota_service::{QuotaService, QuotaServiceDefault};

mod protocol_service;
pub use protocol_service::{ProtocolService, ProtocolServiceDefault};

pub trait Service: TimeService {
    type Randomness: RandomnessService;
    type Dns: DnsService;
    type Mio: MioService;
    type Storage: StorageService;
    type Rpc: RpcService;
    type Actors: ActorsService;
    type Quota: QuotaService;
    type Protocol: ProtocolService;

    fn randomness(&mut self) -> &mut Self::Randomness;

    fn dns(&mut self) -> &mut Self::Dns;

    fn mio(&mut self) -> &mut Self::Mio;

    fn storage(&mut self) -> &mut Self::Storage;

    fn rpc(&mut self) -> &mut Self::Rpc;

    fn actors(&mut self) -> &mut Self::Actors;

    fn quota(&mut self) -> &mut Self::Quota;

    fn protocol(&mut self) -> &mut Self::Protocol;
}

pub struct ServiceDefault {
    pub randomness: RandomnessServiceDefault,
    pub dns: DnsServiceDefault,
    pub mio: MioServiceDefault,
    pub storage: StorageServiceDefault,
    pub rpc: RpcServiceDefault,
    pub actors: ActorsServiceDefault,
    pub quota: QuotaServiceDefault,
    pub protocol: ProtocolServiceDefault,
}

impl TimeService for ServiceDefault {}

impl Service for ServiceDefault {
    type Randomness = RandomnessServiceDefault;
    type Dns = DnsServiceDefault;
    type Mio = MioServiceDefault;
    type Storage = StorageServiceDefault;
    type Rpc = RpcServiceDefault;
    type Actors = ActorsServiceDefault;
    type Quota = QuotaServiceDefault;
    type Protocol = ProtocolServiceDefault;

    fn randomness(&mut self) -> &mut Self::Randomness {
        &mut self.randomness
    }

    fn dns(&mut self) -> &mut Self::Dns {
        &mut self.dns
    }

    fn mio(&mut self) -> &mut Self::Mio {
        &mut self.mio
    }

    fn storage(&mut self) -> &mut Self::Storage {
        &mut self.storage
    }

    fn rpc(&mut self) -> &mut Self::Rpc {
        &mut self.rpc
    }

    fn actors(&mut self) -> &mut Self::Actors {
        &mut self.actors
    }

    fn quota(&mut self) -> &mut Self::Quota {
        &mut self.quota
    }

    fn protocol(&mut self) -> &mut Self::Protocol {
        &mut self.protocol
    }
}
