# The Goal

The goal is to store merkle tree in memory instead of persisting it on
the disk, in order to allow faster reads/writes. However whole merkle tree
can be huge and it will only grow, so we need to only store what we need
in memory and **garbage collect** the rest. We need to maintain data from the
last **5 cycles**  for merkle tree to function correctly.

# How?

Instead of storing all key/values for merkle tree in one store, we use
use multiple stores.

Each store contains only data added during corresponding cycle.

So basically this is how process goes:

  1. We initialize list with empty **5** key-value stores guarded by Read-Write lock.
  1. Have another **current** store which is for current cycle. On each commit
    we push data to this store.
  1. After cycle ends:

      - Move **current** store inside stores list and replace it with empty store.
      - Start garbage collection in GC thread. It basically takes first (oldest)
        store and moves key values that was [reused](#marking-key-value-pair-as-reused)
        by newer cycles to the last (newest) store. Then that old store
        is destroyed and gc is finished.


## Marking Key-Value Pair As Reused

When we reach end of the block and commit changes, we mark every key that
wasn't modified and exists in one of previous cycles (not in the current one)
as **reused**.

**Note:** we only mark roots of tree that is reused, we don't mark its children.
Garbage collector does that work later. This way we preserve memory.

## Concurrency

**Writes** in this setup are completely parallel since **current** store is only
used by main thread, it's not accessable from gc side (we don't need it).

**Reads** don't need to take lock if key exists in **current** store. Otherwise
we need to take **read** lock, to read it from previous cycles. Reads will
only get slower when gc is running (new cycle began) since it takes **write**
lock. Other operations from GC side takes only **read** lock and every operation
from main thread takes **read** lock as well.


# Summary

#### Pros:
  - Fast writes - we operate on smaller in-memory key-value store
    (since it contains data only from the current cycle).
  - Fast reads from current cycle - same reason as above. 
  - Low memory usage - no need to maintain reference counts for example.
  - Little computation required - we know exactly which keys to copy
    and we quickly drop the rest.
#### Cons:
  - Slowdown on reads from previous cycles, when GC is competing
    (taking write lock) at the end of the cycle.
