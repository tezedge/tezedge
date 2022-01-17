The following documentation describes the programming patterns and primitives used in this proof-of-concept code and the rationale behind them.

# Pure/Impure Actions

The current state-machine implementation used by TezEdge is based on “actions/effects/reducers”. The proposed new code also follows this pattern, but with some important changes that will be described next.

In the original implementation (https://blog.nrwl.io/ngrx-patterns-and-techniques-f46126e2b1e5) when an action is “dispatched” it calls the “effects” handler, which as the name suggests it can perform side effects (like IO), and then it calls the “reducer” which applies the action to an state to produce a new state. Given an action and an initial state, calling the reducer is a deterministic operation, while the effects are not.

The effect handlers shouldn’t change the state directly (this is done by the reducer), but what they can do (other than performing IO) is to change the *actions* applying action “deciders” and “transformers”. These changes can be:
- Converting an action from one type to another (filtering decider). This can be based on the action’s contents (content-based decider), or based on the current state (context-based decider).
- Convert an action to several other actions (splitter decider).
- Merge several actions into a single action (aggregator decider).
- Normalise several action types into the same canonical action (normalizer transformer).
- Add more information to the current action (content enricher transformer).

Note that not all actions require performing side effects, but the implementation we just described always calls the “effects” handler. A big portion of the actions used in TezEdge doesn’t require IO effects (these actions are “pure”, or deterministic) and they could be dispatched to the “reducer” directly, without going through “effects”. A problem with the current implementation is that dispatching every action through the “effects” handlers forces us to place a large part of “pure” (deterministic) logic outside the “reducers”.

The first change proposed is to split actions in two kinds: pure, and impure. Pure actions are dispatched directly to the reducers, whereas impure actions will be dispatched to effects. Se we can define actions types of either kind by giving them to the following traits:

```
pub trait ImpureAction<S> {
    fn dispatch_impure(&self, state: &GlobalState, service: &mut S) {
        self.effects(state, service);
    }

    fn effects(&self, state: &GlobalState, service: &mut S);
}

pub trait PureAction {
    fn dispatch_pure(&self, state: &GlobalState) {
        self.reducer(state);
    }

    fn reducer(&self, state: &GlobalState);
}
```

Instead of using a generic `dispatch` name we use `dispatch_impure` and `dispatch_pure` to make it even more explicit. Impure actions can only be dispatched from external services or from effects handlers. Pure actions can be dispatched *anywhere*, including from within the reducers code! This allows us to "compose" reducers while keeping the benefits of actions (recording/logging, debuggability).

Following this pattern we will see that the major part of actions can be implemented as `PureAction` and a very few of them are `ImpureAction`. In the `mio` service we implement just 3 `ImpureAction`s: `MioEventConnectionClosedAction`, `MioCanReadAction` and `MioCanWriteAction`.

The `effects` handler of `MioEventConnectionClosedAction` behaves as a "splitter decider" , converting that single action into to several `CancelTransactionAction` (pure) actions that are dispatched to cancel any transactions (we will cover transactions soon) associated to the `mio::Token` affected by the disconnect event.

https://github.com/tezedge/tezedge/blob/shell_automaton_poc/shell_automaton_poc/src/service/mio.rs#L386

The `effects` handlers of `MioCanReadAction` and `MioCanWriteAction` behave like "context-based deciders", they perform IO (either `recv` or `send` to the socket) and dispatch a `RecvDataAction` (pure) or `SendDataAction` (pure) with IO operation results.

https://github.com/tezedge/tezedge/blob/shell_automaton_poc/shell_automaton_poc/src/service/mio.rs#L432
https://github.com/tezedge/tezedge/blob/shell_automaton_poc/shell_automaton_poc/src/service/mio.rs#L468

*NOTE*: you can see that the method definitions use `&GlobalState` where it should be `&mut GlobalState`, this is because interior mutability has been introduced to enforce some rules about how different parts of the global state can be mutated by reducers. It is a consequence of transactional mechanism that will be explained next, right now you should know that it is not strictly needed (and could be removed later) but it helps to prevent bugs.

# Transactions

We must be careful about state transitions in order to avoid producing invalid (or impossible) states. As a preventive measure we can include *enabling-condition* and *safety-condition* checks for every state transition.

A complementary solution is to use the type system to model data in a way that illegal states are unrepresentable (https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/). Sometimes, the performance cost is too big to allow such implementation and we end up with duplicated instances of values representing some state. In these cases, operations must update all copies of the value atomically, otherwise "intermediary" states (when one copie was updated and others not) can violate the safety-conditions.

An operation (or task) can be composed of multiple actions, and each action changes state. In our current implementation we process one action at a time, so in this sense we could say that atomic units are actions. When we check for enabling or safety conditions we do it per action. However, we currently have some actions that update one copy only (add-to-blacklist example) and leave us in one of these invalid "intermediary" states (until the next action updates the other copy).

Now that we have the concept of `PureAction` a simple, yet general solucion could be to pack several (pure) actions in some kind of `AtomicAction` wrapper, whose `reducer` would just call each of the inner actions' reducers sequentially and perform the safety condition checks after the last `reducer`.

A more general solution that covers operations requiring one or more `ImpureAction`s, is to implement a transactional system where operations can follow some (or all) of the A.C.I.D. properties (https://en.wikipedia.org/wiki/ACID). In the proposed implementation we consider actions as the basic block statements making up a transaction. From the global state point of view, **transactions are atomic**, by using transactions we can combine multiple actions (pure and impure) into a single atomic operation.

To implement a basic transactional system we reorganise the state representation which is now split in 3 areas:

```
pub struct GlobalState {
    pub transactions: RefCell<Transactions>,
    pub shared: SharedState,
    pub immutable: ImmutableState,
}
```

- The `transactions` area contains all the ongoing transactions of the state-machine. Each transaction has an unique ID, and a transaction state can  be either `Pending` (the transaction has started) or `Retry` (the transaction hasn't started yet because it has mutually-exclusive effects with some other `Pending` transaction). When a transaction completes it is just removed from the `transactions` area, every time this happens the state machine checks if a `Retry` transaction can be moved to `Pending` state. Finally, each transaction has a local-state. This local-state is part of the global-state (and observable by any part of the state machine), so what we mean by **local-state** is that this state **can be mutated only** only by actions assigned to that particular transaction.

- The `shared` area is **all** the **mutable state**. Reducers can read from the shared area at any moment, but only the `CompleteTransactionAction` `reducer` can write to it, and does so by calling the transaction's `commit` method. Every transaction must end by dispatching a `CompleteTransactionAction` (pure) action and perform any global-state modifications inside the transaction's `commit` implementation.

- The `immutable` area is read-only state information that can be accessed anywhere.

To sum up, during the life-time of a transaction the transaction's actions can:
- Read anywhere from `GlobalState` (`transactions`, `shared`, and `immutable` areas.
- Mutate the transaction's local-state (which is globally visible).
- Mutate the `shared` state only when the transaction completes, following some permission rules that will be explained next.

## Shared-state access rules

The state machine processes each action at a time, but operations involve multiple actions, and the state machine handles multiple concurrent operations. Potentially, actions could be dispatched in any order, including actions that share access to the same state. This situation can create race-conditions, TOCTOUs, and similar issues. By using transactions we can eliminate all these because of the atomic nature of transactions.

To impose atomicity we must serialise the execution of transactions that have mutually-exclusive access to shared state. The rules here are the same as for an `RwLock`: multiple readers can execute concurrently, but readers can't execute during the execution of a writer, and a writer can't execute if there is at least one reader. For any other case the transactions can execute concurrently.

When we create a transaction we do it by dispatching `CreateTransactionAction` (pure):

```
pub struct CreateTransactionAction<T: Transaction>
where
    Transactions: TransactionsOps<T>,
    T: Clone,
{
    transaction_access: FieldAccess,
    transaction_type: T,
    parent: Option<ParentInfo>,
}
```

In the payload we provide the transaction type and `FieldAccess` information, this information tells the state machine which fields of the `SharedState` are read or written by the transaction.

```
pub struct FieldAccess {
    pub read: FieldAccessMask,
    pub write: FieldAccessMask,
}
```

Currently there is only one field in the `SharedState`, which is the `Peer` list:

```
pub struct SharedState {
    pub peers: SharedField<BTreeSet<Peer>, PEERS_ACCESS_FIELD>,
    // rest of shared state...
}
```

By specifying the `FieldAccessMask` when creating a new transaction, the state machine won't allow it to run if there is any other `Pending` transaction accessing the *same* fields.

Moreover, by making use of `RefCell` and interior mutability, we can impose access restrictions to `reducers` so they can't access state outside the transaction's `FieldAccessMask` of the transaction. For example if a transaction has to write to the `peers` list it can't do it directly because of `&GlobalState`, so it has to do the following:

```
 let mut peers = state.shared.peers.borrow_mut(&state.access(tx_id));
```

Where `tx_id` is the ID of the current transaction. This will only succeed if the `write` `FieldAccessMask` of the transaction has the `PEERS_ACCESS_FIELD` bit set.

## Transaction composition

A very powerful and useful abstraction is the ability to compose multiple transactions. One example is the STM implementation https://en.wikipedia.org/wiki/Software_transactional_memory#Composable_operations

Composing transactions allow us to implement different layers and glue them together. For example we have the "low-level" `TxRecvData` transaction, that completes when the total of the length requested is received (or if the transaction failed). Under the hood there might be multiple `RecvDataAction` actions involved until the transaction completes. Now we want to implement a "chunking" layer, so we implement a `TxRecvChunk` by requesting sequential `TxRecvData` transactions, the first to get the 16-bit chunk size and the second requesting that size. We could then implement transactions for each peer message by chaining multiple `TxRecvChunk` transactions.

The problem with composing transactions comes when any of the inner transactions `commits` because that violates the guarantees of the parent transaction. And we can also lack a "rollback" mechanism if the parent transaction is cancelled after some inner transaction `commit`ed.

Luckly, in our case the only thing that an inner (or child) transaction does on `commit`, is to notify the parent (by dispatching an action to the parent transaction ID). We can classify transactions into kinds: root transactions, and children transactions. Their difference is that root transactions have a `None` parent while children transactions have `Some(ParenInfo)` of the parent transaction (the caller of `CreateTransactionAction`). With this classification we can impose the following rules:

```
    pub fn is_access_exclusive(
        &self,
        parent: Option<ParentInfo>,
        transaction_access: &FieldAccess,
    ) -> bool {
        let transactions_with_access = self.transactions_with_access();
        let mut read_access = 0;
        let mut write_access = 0;

        if let Some(ParentInfo { tx_id, .. }) = parent {
            let ancestors = self.ancestors(tx_id);
            /*
                Children transactions can read into an ancestor's write-field (children complete
                before parent's commit). It could be possible for children transactions to write
                to fields as long as they don't overlap with the ancestor's fields, however this
                would need "rollback" support for transaction cancelation, which might require a
                complex implementation. When children transactions commit, they usually just need
                to notify the parent transaction and don't perform any other side-effects, so we
                just deny children transaction that attempts to write to to shared-state.
            */
            assert!(transaction_access.write == 0);

            // filter out ancestors' access
            for (_, access) in transactions_with_access
                .iter()
                .filter(|(tx_id, _)| ancestors.get(tx_id).is_none())
            {
                read_access |= access.read;
                write_access |= access.write;
            }
        } else {
            for (_, access) in transactions_with_access.iter() {
                read_access |= access.read;
                write_access |= access.write;
            }
        }

        transaction_access.exclusive(&FieldAccess::new(read_access, write_access))
    }
```

In short, children transactions can't have any bit set in their `write` `FieldAccessMask`.

