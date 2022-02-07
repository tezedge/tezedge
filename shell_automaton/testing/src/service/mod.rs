//! Mocked service.

pub use shell_automaton::service::{Service, TimeService};

mod randomness_service;
pub use randomness_service::*;

mod dns_service;
pub use dns_service::*;

mod quota_service;
pub use quota_service::*;

mod storage_service;
pub use storage_service::*;

mod actors_service;
pub use actors_service::*;

mod rpc_service;
pub use rpc_service::*;

mod mio_service;
pub use mio_service::*;

mod protocol_service;
pub use protocol_service::*;

mod protocol_runner_service;
pub use protocol_runner_service::*;

mod websocket_service;
pub use websocket_service::*;
