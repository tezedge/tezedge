Light Node
===========

This is an implementation of a lightweight Tezos node written in Rust. 

# Running the node
Detailed information on how to start the node can be found in the repository [README](../README.md) file 

## Arguments
All arguments and their default values are in the [tezedge.config](./etc/tezedge/tezedge.config) file.
They can be provided also as command line arguments in the same format, in which case they have higher priority than the ones in the config file


### Tezos data dir
The path to directory which will be used to store Tezos-specific data. This is a required argument, and if the node fails to 
create or access this directory, it will die gracefully.

```
--tezos-data-dir <PATH>
```

### Identity file
The path to the json identity file with peer-id, public-key, secret-key and pow-stamp. 
If an identity does not exist in the specified path, a new one will be automatically generated. 
In case it starts with "./" or "../", it is a relative path to the current dir, otherwise to the --tezos-data-dir

```
--identity-file <PATH>
```

### Bootstrap database path
Path to bootstrap database directory. 
In case it starts with "./" or "../", it is a relative path to the current dir, otherwise to the --tezos-data-dir. 
If directory does not exists, it will be created. If directory already exists, and 
contains valid database, node will continue in the bootstrap process on that database

```
--bootstrap-db-path <PATH>
```

### Bootstrap lookup addresses
List of peers to bootstrap the network from. Peers are delimited by a colon. 
For further information, see `--network` parameter of the OCaml node.

```
--bootstrap-lookup-address <ADRRESS>(,<ADDRESS>)*
```

### Logging file
Path to the logger file. If provided, logs are written to the log file, otherwise they are displayed in the terminal. 
In case it starts with "./" or "../", it is a relative path to the current dir, otherwise to the --tezos-data-dir
```
--log-file <PATH>
```

### Logging format
Set format of logger entries, used usually with `--logger-format` argument.
Possible values are either `simple` or `json`.
Simple format is human-readable format while JSON produces structured, easily machine-consumable log entries.

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
Specifies the Tezos environment for this node. Accepted values are: 
`alphanet, babylonnet, babylon, mainnet or zeronet`, where `babylon` and `babylonnet` refer to the same environment.

```
--network <NETWORK>
```
### P2P Port
Specifies port for peer to peer communication.

```
--p2p-port <PORT>
```

### RPC port
The node contains a subset of Tezos node REST API, described in further sections. This argument specifies the port on which
those APIs will be available.

```
--rpc-port <PORT>
```

### WebSocket Access Address
The node exposes various metrics and statistics in real-time through a websocket. This argument specifies the address on which this websocket 
will be accessible.

```
--websocket-address <IP:PORT>
```

### Monitor port
Port on which the TezEdge node monitoring information will be exposed
```
--monitor-port <PORT>
```

### Peers <optional>
Allowed network peers to bootstrap from. This argument is good to use in a controlled testing environmnet.
Each peer is described by its address and port in `IP:PORT` format, delimited by a colon.

```
--peers <IP:PORT>(,<IP:PORT>)*
``` 

### Lower peer threshold
Set minimal peer number, if a running node does not has enough connected peers, peer discovery is enforced.

```
-peer-thresh-low <NUMBER>
```

### Higher peer threshold
Set maximum number of connected peers, a running node will not try to connect to any more peers if this threshold is met.

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
Number of ffi calls, after which the Ocaml garbage collector will be called
```
--ffi-calls-gc-treshold <NUM>
```

### Record flag
Flag for turning record mode on/off
```
--record <BOOL>
```
