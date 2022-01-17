The following documentation describes the programming patterns and primitives used in this proof-of-concept code and the rationale behind them.

## Pure/Impure Actions

The current state-machine implementation used by TezEdge is based on “actions/effects/reducers”. The proposed new code also follows this pattern, but with some important changes that will be described next.

In the original implementation (https://blog.nrwl.io/ngrx-patterns-and-techniques-f46126e2b1e5) when an action is “dispatched” it calls the “effects” handler, which as the name suggests it can perform side effects (like IO), and then it calls the “reducer” which applies the action to an state to produce a new state. Given an action and an initial state, calling the reducer is a deterministic operation, while the effects are not.

The effect handlers shouldn’t change the state directly (this is done by the reducer), but what they can do (other than performing IO) is to change the *actions* applying action “deciders” and “transformers”. These changes can be:
- Converting an action from one type to another (filtering decider). This can be based on the action’s contents (content-based decider), or based on the current state (context-based decider).
- Convert an action to several other actions (splitter decider).
- Merge several actions into a single action (aggregator decider).
- Normalise several action types into the same canonical action (normalizer transformer).
- Add more information to the current action (content enricher transformer).

Note that not all actions require performing side effects, but the implementation we just described always calls the “effects” handler. A big portion of the actions used in TezEdge doesn’t require IO effects (these actions are “pure”, or deterministic) and they could be dispatched to the “reducer” directly, without going through “effects”. A problem with the current implementation is that dispatching every action through the “effects” handlers forces us to place a large part of “pure” (deterministic) logic outside the “reducers”.

The first change proposed is to split actions in two kinds: pure, and impure. Pure actions will be dispatched directly to the reducers, whereas impure actions will be dispatched to effects.

https://github.com/tezedge/tezedge/blob/shell_automaton_poc/shell_automaton_poc/src/action.rs

The biggest implication of this separation is that while “impure” actions can only be dispatched from external services or from effects handlers, “pure” actions can be dispatched *anywhere*, including from within other reducers code! This simplifies the logic, making it easier to follow the program's flow and to test it.

## Transactions

When working with a state machine we must be careful about state transitions in order to avoid producing invalid (or impossible) states. As a preventive measure we can include enabling and/or safety-condition checks for every state transition. Another good measure is to model data types in the way that illegal states are unrepresentable (https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/).

We know that a task can be composed of multiple actions, and each action changes state. In our current implementation we process one action at a time, so in this sense we could say that actions are “atomic” units, whereas the larger tasks are not. Several tasks could be executed concurrently by the state machine, so it is hard to make assumptions about the current state in the context of a task.

Another area of concern is task cancellation, for example if an operation timeouts and the task is cancelled after executing an arbitrary number of actions. These actions have already made changes to the state, and we must make sure everything was left in a consistent state.

The second change proposed in this new implementation is to treat tasks (or operations) as “transactions”, following some (or all) of the A.C.I.D. properties (https://en.wikipedia.org/wiki/ACID).

In the proposed implementation we consider each action as one of the statements making up the transaction. And we reorganise the state-machine global state representation which is now split in 3 areas:
- Transactions area: during the transaction’s life-time each transaction is stored in the transactions area, and each transaction has its own local-state (which is part of the global state). An unique ID is associated with the transaction to allow access to its local-state. Most actions now include this transaction ID and they only perform changes to the local-state.
- Shared area: this is the mutable area of the global state. A transaction can read from the shared state at any time but it can only write (commit) to it when the transaction completes and following a series of rules to do so.
- Immutable area: constant/read-only information, for example node’s configuration.

https://github.com/tezedge/tezedge/blob/shell_automaton_poc/shell_automaton_poc/src/state.rs#L655

The transaction’s local-state provides us with “Isolation” during the transaction execution. If a transaction is cancelled or fails, the transaction is removed and it’s local-state removed without causing any changes to the rest of the state.

Write access to the shared-state should be only performed by the transaction’s “commit” handler which is executed only when the transaction completes. This provides us with “Atomicity”. Some access rules allow us to discriminate which specific parts of the shared state are being read and/or written by a specific transaction and they take care of serialising (by mutual exclusion) transactions with state changes that overlap.

TODO: examples. Transaction composition (root/children transactions).
