Light Node
===========

This is implementation of a lightweight Tezos node written in Rust. 

## Components

The current PoC of the Tezos node consists of multiple components which are yet to be fully implemented:

* **p2p/client:** This is the p2p component responsible for receiving and sending messages within Tezos network. Internally, p2p allocates a component, called a p2p-peer, for each node within the reachable network. The p2p-peer listens to incoming messages from peers and routes requests to individual peers. The p2p/client uses separate components for serialization/deserialization of messages between Tezos message format and Rust structures.

* **storage/db:** In the current phase (PoC), this is a very simplistic in memory DB for storing information about the current state of the blockchain.

* **rpc/server:** the component used for the implementation of Tezedge REST API. Using the nodes REST api, the user is able to get insights about the current state of the node as well as execute commands on the node

The relations between individual components are described in the diagram depicted below.

![Preview1](../docs/images/class_diagram.png)


# Running the node
Detailed information, about how to start the node, is in repository [README](../README.md) file 

## Arguments
All arguments and their default values are in [tezedge.config](./etc/tezedge/tezedge.config) file.
They can be provided also as command line arguments in the same format, in which case they have higher priority than the ones in conifg file


### Tezos data dir
Path to directory which will be used to store Tezos specific data. This is required argument, and if node fails to 
create or access this directory, it will die gracefully.

```
--tezos-data-dir <PATH>
```

### Identity file
Path to the json identity file with peer-id, public-key, secret-key and pow-stamp. 
New identity is automatically generated if it does not exist on specified path. 
In case it starts with "./" or "../", it is relative path to the current dir, otherwise to the --tezos-data-dir

```
--identity-file <PATH>
```

### Bootstrap database path
Path to bootstrap database directory. 
In case it starts with "./" or "../", it is relative path to the current dir, otherwise to the --tezos-data-dir. 
If directory does not exists, it will be created. If directory already exists, and 
contains valid database, node will continue in bootstrap process on that database

```
--bootstrap-db-path <PATH>
```

### Bootstrap lookup addresses
List of peers to bootstrap the network from. Peers are delimited by a colon. 
For further information, see `--network` parameter of OCaml node.

```
--bootstrap-lookup-address <ADRRESS>(,<ADDRESS>)*
```

### Logging file
Path to the logger file. If provided, logs are written to the log file, otherwise displayed in terminal. 
In case it starts with "./" or "../", it is relative path to the current dir, otherwise to the --tezos-data-dir
```
--log-file <PATH>
```

### Logging format
Set format of logger entries, used usually with `--logger-format` argument.
Possible values are either `simple` or `json`.
Simple format is human-readable format, where JSON produce structured, easily machine consumable log entries.

```
--log-format <LOG-FORMAT>
```

### Logging format
Set log level. Possible values are: `critical`, `error`, `warn`, `info`, `debug`, `trace`
```
--log-level <LEVEL>
```

### OCaml logging
Enable OCaml runtime logger.

```
--ocaml-log-enabled <BOOL>
```

### Network
Specify the Tezos environment for this node. Accepted values are: 
`alphanet, babylonnet, babylon, mainnet or zeronet`, where `babylon` and `babylonnet` refer to same environment.

```
--network <NETWORK>
```
### P2P Port
Specify port for peer to peer communication.

```
--p2p-port <PORT>
```

### RPC port
Node contains subset of Tezos node REST API, described in further sections. This argument specifies port, on which
those APIs will be available.

```
--rpc-port <PORT>
```

### WebSocket Access Address
Node expose various metrics and statistics in real-time through websocket. This argument specifies address, on which
will be this websocket accessible.

```
--websocket-address <IP:PORT>
```

### Monitor port
Port on which the Tezedge node monitoring information will be exposed
```
--monitor-port <PORT>
```

### Peers <optional>
Allowed network peers to bootstrap from. This argument is good to use in controlled testing environmnet.
Each peer is described by its address and port in `IP:PORT` format, delimited by a colon.

```
--peers <IP:PORT>(,<IP:PORT>)*
``` 

### Lower peer threshold
Set minimal peer number, if running node does not has enough connected peers, peer discovery is enforced.

```
-peer-thresh-low <NUMBER>
```

### Higher peer threshold
Set maximum number of connected peers, running node will not try to connect to any more peers, if this threshold is met.

```
--peer-thresh-high <NUMBER>
```

### Protocol runner
Path to the protocol runner binary, which is compiled with `tezedge`. 
For example: `./target/debug/protocol-runner`.

```
--protocol-runner <PATH>
```

### Number of ffi calls 
Number of ffi calls, after which will be Ocaml garbage collector called
```
--ffi-calls-gc-treshold <NUM>
```

### Record flag
Flag for turn on/off record mode
```
--record <BOOL>
```


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


# What node can currently do

The two diagrams shown below depict the interaction between objects in a sequential order. The sequence of actions is described in a short sentence written above each diagram.

#### 1. Connect to the Tezos p2p network and listen for incoming connections

![Preview2](../docs/images/bootstrap.png)


#### 2. Return the current head of chain

![Preview4](../docs/images/get_current_head.png)
