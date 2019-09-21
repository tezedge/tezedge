Light Node
===========

This is implementation of a lightweight Tezos node written in Rust. 

## Components

The current PoC of the Tezos node consists of multiple components which are yet to be fully implemented:

* **p2p/client:** This is the p2p component responsible for receiving and sending messages within Tezos network. Internally, p2p allocates a component, called a p2p-peer, for each node within the reachable network. The p2p-peer listens to incoming messages from peers and routes requests to individual peers. The p2p/client uses separate components for serialization/deserialization of messages between Tezos message format and Rust structures.

* **storage/db:** In the current phase (PoC), this is a very simplistic in memory DB for storing information about the current state of the blockchain.

* **rpc/server:** the component used for the implementation of tezos-rs REST API. Using the nodes REST api, the user is able to get insights about the current state of the node as well as execute commands on the node

The relations between individual components are described in the diagram depicted below.

![Preview1](../docs/images/class_diagram.png)


# How to run the node


Run
------------
**Run node** 

```
cargo run --tezos-data-dir /tmp/tezos-data-dir
```

![Preview1](../docs/images/bash_cargo_run.gif)


# RPC API

#### /network/points
Returns connected peers, after successful authentication.

```
curl http://127.0.0.1:18732/network/points
```

![Preview1](../docs/images/bash_network_points.gif)


#### /chains/main/blocks/head
Returns current head.

```
curl http://127.0.0.1:18732/chains/main/blocks/head
```

![Preview1](../docs/images/bash_chains_main_blocks_head.gif)

# Run configuration

**Tezos storage data directory**
(Directory should already exists)
```
--tezos-data-dir /tmp/tezos-data-dir
```

**RPC port**
```
--rpc-port 8732
```

**P2P port**
```
--p2p-port 9732
```

**Bootstrap peers**
```
--peers 127.0.0.1:7777,127.0.0.1:8888
```

**Bootstrap address**

```
--bootstrap-lookup-address boot.tzalpha.net,bootalpha.tzbeta.net
```

**Location to identity.json file**
```
--identity /opt/tezos-env/rust-node/identity.json
```

**Disable JSON messages logging**
```
--log-message-contents false
```

e.g.:
```
cargo run -- --tezos-data-dir /tmp/tezos-data-dir --rpc-port 9998 --p2p-port 7533 --identity ./config/identity.json --logj true --logh false
```


# What node can currently do

The two diagrams shown below depict the interaction between objects in a sequential order. The sequence of actions is described in a short sentence written above each diagram.

#### 1. Connect to the Tezos p2p network and listen for incoming connections

![Preview2](../docs/images/bootstrap.png)


#### 2. Return the current head of chain

![Preview4](../docs/images/get_current_head.png)
