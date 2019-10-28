use shell::shell_channel::BlockApplied;

/// Request/Response to access the Current Head data from RpcActor
#[derive(Debug, Clone)]
pub enum GetCurrentHead {
    Request,
    Response(Option<BlockApplied>),
}