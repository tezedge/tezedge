Light Node
===========

This is implementation of a lightweight Tezos node written in Rust. 

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

## Database configuration
### Bootstrap database path
Path to bootstrap database directory. 
In case it starts with "./" or "../", it is relative path to the current dir, otherwise to the --tezos-data-dir. 
If directory does not exists, it will be created. If directory already exists, and 
contains valid database, node will continue in bootstrap process on that database

```
--bootstrap-db-path <PATH>
```

### (Optional) parameters
```
#Max number of threads used by database configuration. If not specified, then number of threads equal to CPU cores.
--db-cfg-max-threads <NUM>
```

```
#Max open files for database. If specified '-1', means unlimited. Default value: 512
--db-cfg-max-open-files <NUM>
```

-----

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

# Performance and optimization
TODO: write hints for best performance and parameter configuration