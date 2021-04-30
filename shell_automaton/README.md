
## Action

Object representing some sort of action.

All such actions are grouped into one global enum:
```rust
enum Action {
    PeersDnsLookupInit(PeersDnsLookupInitAction),
    PeersDnsLookupError(PeersDnsLookupErrorAction),
    PeersDnsLookupSuccess(PeersDnsLookupSuccessAction),
    PeersDnsLookupFinish(PeersDnsLookupFinishAction),

    PeerConnectionOutgoingInit(PeerConnectionOutgoingInitAction),
    PeerConnectionOutgoingPending(PeerConnectionOutgoingPendingAction),
    PeerConnectionOutgoingError(PeerConnectionOutgoingErrorAction),
    PeerConnectionOutgoingSuccess(PeerConnectionOutgoingSuccessAction),
}
```

## Reducer

Responsible for state management.

Takes current `State` and an `Action` and computes new `State`.

Pseudocode: `reducer(state, action) -> state`

We don't really need to take state immutably, in JavaScript/frontend
it makes sense, but in our case, it doesn't.

So reducer now looks like this:
```rust
type Reducer<State, Action> = fn(&mut State, &Action);
```

## Middleware/Effects(side-effects)

Responsible for control flow and other kinds of side-effects.

`Middleware` intercepts every action and does something with it.

It has access to global `State`, as well as services.
It can't mutate `Action` or the `State` directly. It can however issue
another action, which can in turn mutate the state.

```rust
type Middleware<State, Service, Action> = fn(&mut Store<State, Service, Action>, &Action);
```

## Service

Service is an abstraction over the dependencies. Example of services
includes `mio`, `randomness`, `time`, `storage`, etc...

- No state should be included in the `Service`! As a rule of thumb,
  anything that can be serialized, should go inside our global `State`.

## State

Our global state.

## Store

Provided by the framework. Responsible for executing reducers and middlewares.
