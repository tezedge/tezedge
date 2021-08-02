# General Architecture

```
              (EVENTS)---------> Proposer ---proposal---> Acceptor
               ^    ^              |                       |
               |    |              '--------get requests-->|
               |    |              |                       |
---------------'    |              '<-------requests-------'
|                   |              |
|   Network ->------'              |
|    ^                             |
|    '-----network requests------<-'
|                                  |
|                                  |
'<- Storage <--storage requests--<-'
```

# Mio

To understand design used for Tezedge state machine, it's very
important to understand how [mio](https://docs.rs/mio/0.7.13/mio/) works.

From **mio** docs:
> Mio is a fast, low-level I/O library for Rust focusing on non-blocking
> APIs and event notification for building high performance I/O apps with
> as little overhead as possible over the OS abstractions.

With new architecture, **mio** is used for asynchronous p2p communication,
but technically with this architecture, any backend can be used, which
can provide similar interface.

The way mio works is actually pretty simple. It simply wraps around
[std::net::TcpStream](https://doc.rust-lang.org/std/net/struct.TcpStream.html)
and calls [TcpStream::set_nonblocking(true)](https://doc.rust-lang.org/std/net/struct.TcpStream.html#method.set_nonblocking).

This simply means that whenever we try to **read** or **write** to the stream,
unlike default behavior, it won't block until read/write is finished.

In the case of read, on each call it will directly read from kernel's
receive buffer and once it's empty, instead of blocking, it will return
an error: [io::ErrorKind::WouldBlock](https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WouldBlock).

Same for write, when we fill kernel's send buffer, it will return that
same error.

That error is an indication that no further progress can be made at a
given point, hence the resource is **exhausted**.

To make further progress asynchronously and efficiently, we need to
be notified, when there will be further progress to be made. The way
it's done in mio is using [mio::Event](https://docs.rs/mio/0.7.13/mio/event/struct.Event.html).

Event which we receive from mio will tell us about where more progress
can be made:

- `Event::is_readable()` - when more data can be read from kernel.
- `Event::is_writable()` - when more data can be written to kernel.

Important is to remember that we to exhaust readable and writable
resource or we need to remember that stream is ready, because we won't
receive another event from mio until we exhaust the resource.

# Tezedge state machine and actor system

The goal is to remove riker(actor system) completely, but until we
move/convert everything to the state machine, we need to make node work.

Right now handshake and handling of some messages is implemented inside
state machine, so we need some way to connect that to the rest of the
actor system.

That is what `PeerManager` actor is used for. It used to handle handshake
and other parts, but now it's simply used as a shell. All it's contents
are deleted and it is simply used to connect state machine to the actor
system.

At the start it simply spawns thread for state machine. Then it communicates
with that thread using [mpsc channel](https://doc.rust-lang.org/std/sync/mpsc/index.html).
That mpsc channel is used for one way messaging from `PeerManager` to
state machine. It is used to propagate `NetworkChannelMsg` to state machine.

Other actors use `NetworkChannel` to send messages to the state machine.
`PeerManager` receives it and passes it to state machine's thread. When
state machine receives the message from peer (which it doesn't handle yet),
it publishes that message over the network channel.

# Tezedge proposer

`TezedgeProposer` wraps around `TezedgeState` and it's goal to connect
state machine to the parts that might be the source of non-determinism.

This way we create a separation between deterministic state machine and
the rest of the system. So for example if we want to implement record/replay
functionality, all we have to do is intercept this communication
between `TezedgeProposer` and `TezedgeState`, which is done using proposals.

is to interpret
and handle mio events and send corresponding proposals to the `TezedgeState`.

It's goal is to also execute requests coming from `TezedgeState`. Those
requests are usually for outside world (actor system, mio), for example
request might be to notify actor system that handshake was successful,
message received from peer. Or request might be to ask mio to start
listening for new connections, stop listening, connect to peer, etc...

# Tezedge state

`TezedgeState` is the deterministic state machine. It's only input is
proposal. `Proposal` is simply a message/command to state machine, as a
side effect to the proposal, state machine might change it's state.
The only way state machine mutates it's state is through proposal, which
is why exact same proposals in the exact same order will lead us to the
exact same state.

`TezedgeState` implements `Acceptor<Proposal>`, for different kinds of proposals,
which are basically handlers for those proposals.

# Determinism

In the state machine, nothing can be random or current time dependant,
otherwise it won't be deterministic (same behavior won't be observed
on same input).

One of the biggest source of non-determinism is **time**. That is what
proposals are for. They need to include current time within them, that
way we have full control over time and same set of proposals will give
us exact same state. Inside the state machine, you won't be able to find
a place requesting current time `Instant::now()`, instead time inside
proposal is used.

We still need some randomness inside the state machine, for example
when generating nonce, when choosing which peers to advertise to
remote peer, etc...

Right now all of that is wrapped inside `Effects`. At the moment `Effects`
is part of state machine, which is not great, because that means that
same set of proposals won't give us same state, we need to guarantee
same set of effects as well.

That can work, but needlessly complicates design and implementation.
Instead a better approach will be to include `Effects` in the `TezedgeProposer`
and pass that inside proposal. That way design, implementation, simulation
and testing will be simpler. Goal is to switch to that approach eventually.
