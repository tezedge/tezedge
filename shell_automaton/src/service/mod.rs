// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use redux_rs::TimeService;

pub mod service_async_channel;
pub mod service_channel;

pub mod dns_service;
pub use dns_service::{DnsService, DnsServiceDefault};

pub mod randomness_service;
pub use randomness_service::{RandomnessService, RandomnessServiceDefault};

pub mod mio_service;
pub use mio_service::{MioService, MioServiceDefault};

mod protocol_runner_service_worker;

pub mod protocol_runner_service;
pub use protocol_runner_service::{ProtocolRunnerService, ProtocolRunnerServiceDefault};

pub mod storage_service;
pub use storage_service::{StorageService, StorageServiceDefault};

pub mod rpc_service;
pub use rpc_service::{RpcService, RpcServiceDefault};

pub mod actors_service;
pub use actors_service::{ActorsService, ActorsServiceDefault};

pub mod baker_service;
pub use baker_service::{BakerService, BakerServiceDefault};

pub mod statistics_service;
pub use statistics_service::{BlockApplyStats, BlockPeerStats, StatisticsService};

pub trait Service: TimeService {
    type Randomness: RandomnessService;
    type Dns: DnsService;
    type Mio: MioService;
    type Storage: StorageService;
    type ProtocolRunner: ProtocolRunnerService;
    type Rpc: RpcService;
    type Actors: ActorsService;
    type Baker: BakerService;

    fn randomness(&mut self) -> &mut Self::Randomness;

    fn dns(&mut self) -> &mut Self::Dns;

    fn mio(&mut self) -> &mut Self::Mio;

    fn storage(&mut self) -> &mut Self::Storage;

    fn protocol_runner(&mut self) -> &mut Self::ProtocolRunner;

    fn rpc(&mut self) -> &mut Self::Rpc;

    fn actors(&mut self) -> &mut Self::Actors;

    fn prevalidator(&mut self) -> &mut Self::ProtocolRunner;

    fn baker(&mut self) -> &mut Self::Baker;

    fn statistics(&mut self) -> Option<&mut StatisticsService> {
        None
    }
}

pub struct ServiceDefault {
    pub randomness: RandomnessServiceDefault,
    pub dns: DnsServiceDefault,
    pub mio: MioServiceDefault,
    pub storage: StorageServiceDefault,
    pub protocol_runner: ProtocolRunnerServiceDefault,
    pub rpc: RpcServiceDefault,
    pub actors: ActorsServiceDefault,
    pub baker: BakerServiceDefault,
    pub statistics: Option<StatisticsService>,
}

impl TimeService for ServiceDefault {}

impl Service for ServiceDefault {
    type Randomness = RandomnessServiceDefault;
    type Dns = DnsServiceDefault;
    type Mio = MioServiceDefault;
    type Storage = StorageServiceDefault;
    type ProtocolRunner = ProtocolRunnerServiceDefault;
    type Rpc = RpcServiceDefault;
    type Actors = ActorsServiceDefault;
    type Baker = BakerServiceDefault;

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

    fn protocol_runner(&mut self) -> &mut Self::ProtocolRunner {
        &mut self.protocol_runner
    }

    fn rpc(&mut self) -> &mut Self::Rpc {
        &mut self.rpc
    }

    fn actors(&mut self) -> &mut Self::Actors {
        &mut self.actors
    }

    fn prevalidator(&mut self) -> &mut Self::ProtocolRunner {
        self.protocol_runner()
    }

    fn baker(&mut self) -> &mut Self::Baker {
        &mut self.baker
    }

    fn statistics(&mut self) -> Option<&mut StatisticsService> {
        self.statistics.as_mut()
    }
}
