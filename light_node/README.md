Light Node
===========

This is an implementation of a lightweight Tezos node written in Rust. 

# Running the node
Detailed information on how to start the node can be found in the repository [README](../README.md) file 

## Arguments
All arguments and their default values can be found in the [tezedge.config](./etc/tezedge/tezedge.config) file.
They can also be provided as command line arguments in the same format, in which case they have higher priority than the ones in config file


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

## Database configuration
### Bootstrap database path
Path to the bootstrap database directory. 
In case it starts with "./" or "../", it is a relative path to the current dir, otherwise to the --tezos-data-dir. 
If the directory does not exist, it will be created. If directory already exists and 
it contains a valid database, the node will continue in the bootstrapping process on that database

```
--bootstrap-db-path <PATH>
```

### (Optional) parameters
```
#Max number of threads used by database configuration. If not specified, then number of threads will be equal to number of CPU cores.
--db-cfg-max-threads <NUM>
```

-----

### Bootstrap lookup addresses
List of peers to bootstrap the network from. Peers are delimited by a colon. 
For further information, see `--network` parameter of the OCaml node.

```
--bootstrap-lookup-address <ADRRESS>(,<ADDRESS>)*
```

### Logging file
Path to the logger file. If provided, logs are written to the log file, otherwise they will be displayed in the terminal. 
In case it starts with "./" or "../", it is a relative path to the current dir, otherwise to the --tezos-data-dir
```
--log-file <PATH>
```

### Logging format
Set format of logger entries, used usually with `--logger-format` argument.
Possible values are either `simple` or `json`.
Simple format is a human-readable format while JSON produces structured, easily machine-consumable log entries.

```
--log-format <LOG-FORMAT>
```

### Logging level
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
`alphanet, babylonnet, babylon, mainnet, zeronet, carthagenet, carthage, delphinet, delphi, edonet, edo, sandbox`

```
--network <NETWORK>
```
### P2P Port
Specifies port for peer to peer communication.

```
--p2p-port <PORT>
```

### RPC port
The node contains a subset of the Tezos node's REST API as described in further sections. This argument specifies the port on which
those APIs will be available.

```
--rpc-port <PORT>
```

### WebSocket Access Address
The node exposes various metrics and statistics in real-time through a websocket. This argument specifies the address at which this websocket will be accessible.

```
--websocket-address <IP:PORT>
```

### Peers <optional>
Allowed network peers to bootstrap from. This argument is good to use in a controlled testing environmnet.
Each peer is described by its address and port in `IP:PORT` format, delimited by a colon.

```
--peers <IP:PORT>(,<IP:PORT>)*
``` 

### Lower peer threshold
Set minimal number of peers, if the running node does not have enough connected peers, peer discovery is enforced.

```
-peer-thresh-low <NUMBER>
```

### Higher peer threshold
Set maximum number of connected peers. If this threshold is met, then the running node will not try to connect to any more peers.

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
Number of ffi calls, after which the Ocaml garbage collector will be called.
```
--ffi-calls-gc-treshold <NUM>
```

### Bootstrap DNS lookup
Disables DNS lookup to get peers to bootstrap from the network. Default: false
```
--disable-bootstrap-lookup
```
### Mempool
Enable or disable mempool.
```
--disable-mempool
```

### Private node mode
Enable or disable the private node. Use peers to set the IP addresses of the peers you want to connect to.
```
--private-node
```

### Test chain
Flag for enable/disable test chain switching for block applying. Default: false
```
--enable-testchain <BOOL>
```

### Ffi connection pool max connections
Max number of FFI pool connections. default: 10
```
--ffi-pool-max-connections <NUM>
--ffi-trpap-pool-max-connections <NUM>
--ffi-twcap-pool-max-connections <NUM>
```

### Ffi connection timeout
Number of seconds to wait for connection. default: 60
```
--ffi-pool-connection-timeout-in-secs <NUM>
--ffi-trpap-pool-connection-timeout-in-secs <NUM>
--ffi-twcap-pool-connection-timeout-in-secs <NUM>
```

### Ffi pool lifetime
Number of seconds to remove protocol_runner from pool, default: 21600 (6 hours).
```
--ffi-pool-max-lifetime-in-secs <NUM>
--ffi-trpap-pool-max-lifetime-in-secs <NUM>
--ffi-twcap-pool-max-lifetime-in-secs <NUM>
```

### Ffi pool unused timeout
Number of seconds to remove unused protocol_runner from pool, default: 1800 (30 minutes).
```
--ffi-pool-idle-timeout-in-secs <NUM>
--ffi-trpap-pool-idle-timeout-in-secs <NUM>
--ffi-twcap-pool-idle-timeout-in-secs <NUM>
```

### Recording context actions
Activate recording of context storage actions.
```
--store-context-actions 
```

### Sandbox context patching
Path to the json file with key-values which will be added to the empty context on startup and commit genesis.
```
--sandbox-patch-context-json-file <PATH>
```

# Performance and optimization
TODO: write hints for best performance and parameter configuration
