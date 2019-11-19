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
Detailed information, about how to start the node, is in repository [README](../README.md) file.

## Arguments
### Bootstrap database path
Path to bootstrap database directory. If directory does not exists, it will be created. If directory already exists, and 
contains valid database, node will continue in bootstrap process on that database

```
-B, --bootstrap-db-path <PATH>
```
### Bootstrap lookup addresses
List of peers to bootstrap the network from. Peers are delimited by a colon. 
For further information, see `--network` parameter of OCaml node.

```
-b, --bootstrap-lookup-address <ADRRESS>(,<ADDRESS>)*
```
### Identity 
Path to your Tezos `identity.json` file.

```
-i, --identity <PATH>
```
### Logging file
Path to the logger file. By default, all logs are printed on `STDOUT`.

```
-F, --log-file
```
### Logging format
Set format of logger entries, used usually with `--logger-format` argument.
Possible values are either `simple` or `json`.
Simple format is human-readable format, where JSON produce structured, easily machine consumable log entries.

```
-f, --log-format
```
### Network
Specify the Tezos environment for this node. Accepted values are: 
`alphanet, babylonnet, babylon, mainnet or zeronet`, where `babylon` and `babylonnet` refer to same environment.

```
-n, --network [alphanet, babylonnet, babylon, mainnet, zeronet]
```
### OCaml loggin
Enable OCaml runtime logger.

```
-o, --ocaml-log-enabled [true,false]
```
### P2P Port
Specify port for peer to peer communication. Default: `9732`

```
-l, --p2p-port <PORT>
```
### Lower peer threshold
Set minimal peer number, if running node does not has enough connected peers, peer discovery is enforced.
Default: `2`

```
-T, -peer-thresh-low <NUMBER>
```
### Higher peer threshold
Set maximum number of connected peers, running node will not try to connect to any more peers, if this threshold is met.
Default: `15`.

```
-t, --peer-thresh-high <NUMBER>
```
### Peers
Allowed network peers to bootstrap from. This argument is good to use in controlled testing environmnet.
Each peer is described by its address and port in `IP:PORT` format, delimited by a colon.

```
-p, --peers <IP:PORT>(,<IP:PORT>)*
``` 
### Protocol runner
Path to the protocol runner binary, which is compiled with `tezedge`. 
For example: `./target/debug/protocol-runner`.

```
-P, --protocol-runner <PATH>
```
### RPC port
Node contains subset of Tezos node REST API, described in further sections. This argument specifies port, on which
those APIs will be available. Default: `18732`.

```
-r, --rpc-port <PORT>
```
### Tezos data dir
Path to directory which will be used to store Tezos specific data. This is required argument, and if node fails to 
create or access this directory, it will die gracefully. Default: `tezos_data_db`

```
-d, --tezos-data-dir <PATH>
```
### WebSocket Access Address
Node expose various metrics and statistics in real-time through websocket. This argument specifies address, on which
will be this websocket accessible. Default: `0.0.0.0:4972`.

```
-w, --websocket-address <0.0.0.0:PORT>
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
