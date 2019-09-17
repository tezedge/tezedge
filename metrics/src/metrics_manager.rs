use tokio::TaskExecutor;
use std::collections::HashMap;
use riker::actor::{ActorUri, ActorRefFactory, Props,Context};

pub struct MetricsManager {
    executor: TaskExecutor,
    ws_port: u16,
    protocol_name: String,
    peers: HashSet<ActorUri>,
    clients: HashMap<>
}

impl MetricsManager {
    pub fn actor(sys: &impl ActorRefFactory, executor, ws_port, protocol_name) {
        sys.actor_of(Props::new_args(Self::new, (executor, ws_port, protocol_name)), Self::name())
    }

    fn name() -> &'static str {
        "metrics-manager"
    }

    fn new((executor, ws_port, protocol_name): (TaskExecutor, u16, String)) -> Self {
        Self {
            executor,
            ws_port,
            protocol_name,
            peers: HashSet::with_capacity(15),  // TODO: map this to threshold for better sub-optimal results
        }
    }
}

impl Actor for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        let myself = ctx.myself();

        self.executor.spawn(async move {
            handle_ws_connections(self.ws_port.clone(), ctx.myself()).await;
        });
    }
}
