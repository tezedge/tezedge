
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

    ..
}
```
link to definition of all actions: [shell_automaton::Action](src/action.rs)

## Reducer

Responsible for state management. Only function that's able to change
the state is the reducer.

Takes current `State` and an `Action` and computes new `State`.

Pseudocode: `reducer(state, action) -> state`

We don't really need to take state immutably, in JavaScript/frontend
it makes sense, but in our case, it doesn't.

So reducer now looks like this:
```rust
type Reducer<State, Action> = fn(&mut State, &Action);
```

Main reducer function that gets called on every action:
[shell_automaton::reducer](src/reducer.rs)

## Effects(side-effects)

Responsible for control flow and other kinds of side-effects.

`Effects` run after every action and triggers side-effects (calls to the
service or dispatches some other action).

It has access to global `State`, as well as services.
It can't mutate `Action` or the `State` directly. It can however dispatch
another action, which can in turn mutate the state.

```rust
type Effects<State, Service, Action> = fn(&mut Store<State, Service, Action>, &Action);
```

Main effects function that gets called on every action:
[shell_automaton::effects](src/effects.rs)

## Service

Service is an abstraction over the external dependencies, logic that
isn't included in `shell_automaton`. For example IO, interacting with kernel,
time, etc... Basically any sort kinds of sources of input.

- No state should be included in the `Service`! As a rule of thumb,
  anything that can be serialized, should go inside our global `State`.

## State

Our global state. [shell_automaton::State](src/state.rs)

## Store

Provided by the framework. Responsible for executing reducers and effects.

Has a main method `dispatch`, which calls the `reducer` with the given action
and calls `effects` after it.
