use riker::actors::*;
use crate::peer::PeerRef;

/// Peer has been created. This event does indicate
/// only creation of the peer and is not indicative if
/// bootstrap is going to be successful or not.
#[derive(Clone, Debug)]
pub struct PeerCreated {
    pub peer: PeerRef
}

/// The peer has disconnected.
#[derive(Clone, Debug)]
pub struct PeerDisconnected;

/// Peer has been bootstrapped.
#[derive(Clone, Debug)]
pub struct PeerBootstrapped;

/// Network channel event message.
#[derive(Clone, Debug)]
pub enum NetworkChannelMsg {
    PeerCreated(PeerCreated),
    PeerDisconnected(PeerDisconnected),
    PeerBootstrapped(PeerBootstrapped)
}

impl From<PeerCreated> for NetworkChannelMsg {
    fn from(msg: PeerCreated) -> Self {
        NetworkChannelMsg::PeerCreated(msg)
    }
}

impl From<PeerDisconnected> for NetworkChannelMsg {
    fn from(msg: PeerDisconnected) -> Self {
        NetworkChannelMsg::PeerDisconnected(msg)
    }
}

impl From<PeerBootstrapped> for NetworkChannelMsg {
    fn from(msg: PeerBootstrapped) -> Self {
        NetworkChannelMsg::PeerBootstrapped(msg)
    }
}

pub const DEFAULT_TOPIC: &str = "network";

/// This struct represents network bus where all network events must be published.
pub struct NetworkChannel(Channel<NetworkChannelMsg>);

impl NetworkChannel {

    fn props() -> BoxActorProd<Channel<NetworkChannelMsg>> {
        Props::new(Channel::new)
    }

    fn name() -> &'static str {
        "network-channel"
    }

    pub fn channel(fact: &impl ActorRefFactory) -> Result<ChannelRef<NetworkChannelMsg>, CreateError> {
        fact.actor_of(NetworkChannel::props(), NetworkChannel::name())
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl Actor for NetworkChannel {
    type Msg = ChannelMsg<NetworkChannelMsg>;

    fn pre_start(&mut self, ctx: &ChannelCtx<NetworkChannelMsg>) {
        self.0.pre_start(ctx);
    }

    fn recv(
        &mut self,
        ctx: &ChannelCtx<NetworkChannelMsg>,
        msg: ChannelMsg<NetworkChannelMsg>,
        sender: Sender,
    ) {
        self.0.receive(ctx, msg, sender);
    }
}
