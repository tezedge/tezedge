### Action Recording

The tezedge node can be used for recording ContextAction that was executed by the node. By default, all the actions are stored in RocksDB database, but also file can be use as a storage.
There is a dedicated node parameter called `actions-store-backend` that can be used for specifying the desired 'action storage backend'

```
    --actions-store-backend <STRING>...
        Activate recording of context storage actions [possible values: none, rocksdb, file]
```

One can start action recording by running the node:

```
./run.sh node 
    --tezos-data-dir /tmp/light_node/tezos
    --bootstrap-db-path /tmp/light_node/tezedge
    --actions-store-backend file
    --network=<NETWORK>
```

there is one extra flag that might be useful:

```
--compute-context-action-tree-hashes=true
```

once set ContextActions will include `new_tree_hash` field which is expected context hash *after executing the action*


File with already recorded actions can be found in 
```
/tmp/light_node/tezedge/actionfile.bin
```
With every processed block its appended at the end of the file - there is no rewriting  of already existing data.

Actions in the file are stored as:

```
|block1 len in bytes||action 1| action 2| ... | aciton N||block2 len in bytes||action 1| action 2| ... | aciton N|
<---block 1 header--><-------------block1---------------><---block 2 header--><-------------block2--------------->
```

where :
 - `header N` - is unsigned int 32 - lenght of the `N` block
 - `block N` - Vec<[ContextAction](https://github.com/tezedge/tezedge/blob/develop/tezos/context/src/channel.rs#L44)> serialized using [bincode crate](https://docs.rs/bincode/1.3.2/bincode/) and then compress  using [snap](https://crates.io/crates/snap)

single block is represented as `Vec<ContextAction>`


There is dedicated [ActionFileReader](https://github.com/tezedge/tezedge/blob/develop/storage/src/action_file.rs#L61) that can be used for reading and deserializing following blocks





