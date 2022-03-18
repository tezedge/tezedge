# Node monitoring

Application for monitoring a running node and it's consumption of system resources

## Prerequisites

Only works on linux like systems because it heavily uses the /proc filesystem 

## Options

- `tezedge-nodes`: An array of tezedge type nodes separated by space in the following notation: \<node_tag\>:\<node_rpc_port\>:\<path to node database\>
- `ocaml-nodes`:(Optional) An array of octez type nodes separated by space in the following notation: \<node_tag\>:\<node_rpc_port\>:\<path to node database\>
- `debugger-path`: (Optional) Specify the path to the debugger database to monitor the size of the debugger database for the node. 
- `rpc-port`: (Optional) Custom port for the resources endpoints. Defaults to 38732.
- `slack-url`: (Optional) Slack webhook url used to send messages to specific channels
- `slack-token`: (Optional) Token used to upload log file on node crash
- `slack-channel-name`: (Optional) Monitoring channel name.
- `resource-monitor-interval`: (Optional) Sets the interval in seconds in which the module performs resource monitoring. Defaults to 5 seconds 
- `tezedge-alert-threshold-memory`: (Optional) Sets an alert threshold in MB for memory consumption. Defaults to 4096.
- `tezedge-alert-threshold-synchronization`: (Optional) Sets a threshold in seconds to report a stuck node. If the node fails to update it's current head in this threshold, the node is pronounced stuck. Defaults to 300.
- `tezedge-alert-threshold-cpu`: (Optional) The usage threshold in %. After passing this threshold the high cpu usage is reported as an alert. Disabled by default
- `tezedge-alert-threshold-disk`: (Optional) The threshold in(0-100)%. This measurement calculates the used space on the filesystem and reports when it has passed the set threshold. Defaults to 95.
- `ocaml-alert-threshold-disk`: (Optional) The threshold in(0-100)%. This measurement calculates the used space on the filesystem and reports when it has passed the set threshold. 
- `ocaml-alert-threshold-memory`: (Optional) Sets an alert threshold in MB for memory consumption. Defaults to 6144
- `ocaml-alert-threshold-cpu`: (Optional) The usage threshold in %. After passing this threshold the high cpu usage is reported as an alert. Disabled by default.
- `ocaml-alert-threshold-synchronization`: (Optional) Sets a threshold in seconds to report a stuck node. If the node fails to update it's current head in this threshold, the node is pronounced stuck. Defaults to 300s.
- `proxy-port`: (Optional) An additional port to monitor in case of using a proxy
- `wait-for-nodes`: (Optional) Wait for the defined nodes. Useful inside docker containers when you have to wait for the nodes to start

## The node notation

```
<node_tag>:<node_rpc_port>:<path_to_node_database>
```

The way to inform the monitoring about the nodes to monitor is to use the above notation.

- `node_tag`: an arbitrary tag of our choice. This is important, because the monitoring opens this path on it's rpc server. E.g.: for node tag `my_tezedge_node` the resources will be accessible on http://localhost:38732/resources/my_tezedge_node
- `node_rpc_port`: The node's rpc port that it opened/will open
- `path_to_node_database`: Path to the node database used for monitoring the disk size.

## Running the monitoring

1. Build from sources:

    ```
    cd /apps/node_monitoring && SODIUM_USE_PKG_CONFIG=1 cargo build --release
    ```

2. Run alongside an already running node with slack notifications enabled:

    ```
    sudo ./target/release/node-monitoring --tezedge-nodes tezedge:18888:../tmp/tezedge --resource-monitor-interval 1 --slack-channel-name random --slack-url "https://hooks.slack.com/services/XXXXXXXXX/XXXXXXXXXX/XXXXXXXXXXXXXXXXXXXXXX" --slack-token "Bearer xoxb-XXXXXXXXXXX-XXXXXXXXXXXX-XXXXXXXXXXXXXXXXXXXXXXX"
    ```

## RPC response breakdown

Call example: http://116.202.128.230:38732/resources/tezedge

```
{
    "timestamp": Epoch timestamp,
    "memory": {
        "node": Integer,
        "validators": ValidatorMemoryData
    },
    "disk": {
        "context_irmin": Integer,
        "context_merkle_rocksdb": Integer,
        "block_storage": Integer,
        "context_actions": Integer,
        "main_db": Integer,
        "debugger": Integer
    },
    "cpu": {
        "node": CpuData
        "validators": ValidatorCpuData
    },
    "io": {
        "node": IOData,
        "validators": ValidatorIOData
    },
    "network": NetworkData
}
```

### ValidatorMemoryData

An object that represents validator memory data. `total` represents the memory usage of all the vlaidators combined. `validators` is a map that contains all the validator process' memory usage one by one. 

```
"total": Integer,
"validators": ValidatorMemoryMap
```

### ValidatorMemoryMap

A map that contains all the validator process' memory usage one by one
```
"protocol-runner-861624": Integer,
"protocol-runner-861587": Integer
...
...
...
```

### CpuData

An Object that represents the CPU usage of the process. The *collective* represents the collective CPU usage of the process (the main running thread + all task threads). *task_threads* is represented by another Object, ThreadData. 

```
"collective": Float,
"task_threads": ThreadData
```

### ValidatorCpuData

Same as ValidatorMemoryData only with Cpu data.

```
"total": Integer,
"validators": ValidatorCpuMap
```

### ValidatorCpuMap

```
"collective": Float,
"task_threads": ThreadData
```

### ThreadData

An Object that represents all the threads spawned by the process and their CPU usage. Each thread name is a property and the value is its CPU usage
```
"506567-ctrl-c": Float,
"506480-protocol-runner": Float,
"506482-ffi-ocaml-execu": Float,
"506566-protocol-runner": Float,
"506708-ffi-ocaml-execu": Float,
"506481-ctrl-c": Float,
"510114-ffi-ocaml-execu": Float,
"506568-ffi-ocaml-execu": Float
...
...
...
```

### IOData

Representing the disk IO data for the process in read and wirtten amount of data notation. (Bytes per seconds)
```
"read_bytes_per_sec": Integer,
"written_bytes_per_sec": Integer
```

### ValidatorIOData

Same as ValidatorMemoryData only with IO data.
```
"total": Integer,
"validators": ValidatorIOMap
```

### ValidatorIOMap

```
"protocol-runner-861587": IOData,
"protocol-runner-861624": IOData,
...
...
...
```

### Network data

Representing the network usage of the node in sent and received amount of Bytes notation. (Bytes per seconds)
```
"sent_bytes_per_sec": Integer,
"received_bytes_per_sec": Integer
```


## Example

```
{
    "timestamp": 1627427269,
    "memory": {
        "node": 160354304,
        "validators": {
            "total": 720175104,
            "validators": {
                "protocol-runner-861624": 108920832,
                "protocol-runner-861587": 611254272
            }
        }
    },
    "disk": {
        "contextIrmin": 60411424,
        "contextMerkleRocksdb": 0,
        "blockStorage": 19489250,
        "contextActions": 0,
        "mainDb": 79667220,
        "debugger": 19778460666
    },
    "cpu": {
        "node": {
            "collective": 21.761658,
            "taskThreads": {
                "r2d2-worker-2-861557": 0.0,
                "r2d2-worker-2-861551": 0.0,
                "r2d2-worker-2-861560": 0.0,
                "r2d2-worker-0-861549": 0.0,
                "r2d2-worker-0-861552": 0.0,
                "pool-thread-#4-861565": 0.77720207,
                "light-node-861536": 0.0,
                "pool-thread-#21-861582": 0.0,
                "pool-thread-#11-861572": 0.77720207,
                "r2d2-worker-1-861556": 0.0,
                "pool-thread-#3-861564": 0.0,
                "pool-thread-#22-861583": 0.77720207,
                "rocksdb:low5-861511": 0.0,
                "pool-thread-#13-861574": 0.77720207,
                "tokio-runtime-w-861540": 0.0,
                "pool-thread-#19-861580": 0.77720207,
                "tokio-runtime-w-861546": 0.77720207,
                "tokio-runtime-w-861547": 1.5544041,
                "tokio-runtime-w-861541": 0.0,
                "tokio-runtime-w-861548": 0.0,
                "rocksdb:low11-861517": 0.0,
                "pool-thread-#1-861562": 0.77720207,
                "light-node-861505": 0.0,
                "pool-thread-#17-861578": 0.0,
                "pool-thread-#9-861570": 0.77720207,
                "rocksdb:low2-861508": 0.0,
                "tokio-runtime-w-861538": 1.5544041,
                "tokio-runtime-w-861544": 0.0,
                "pool-thread-#15-861576": 0.0,
                "rocksdb:low10-861516": 0.0,
                "r2d2-worker-0-861558": 0.0,
                "r2d2-worker-1-861559": 0.0,
                "rocksdb:high2-861520": 0.0,
                "chain-feedr-ctx-861586": 5.4404144,
                "pool-thread-#2-861563": 0.0,
                "r2d2-worker-2-861554": 0.0,
                "rocksdb:low8-861514": 0.0,
                "light-node-861585": 0.0,
                "tokio-runtime-w-861537": 0.0,
                "pool-thread-#18-861579": 0.0,
                "rocksdb:low3-861509": 0.0,
                "pool-thread-#12-861573": 0.0,
                "r2d2-worker-0-861555": 0.0,
                "pool-thread-#10-861571": 0.77720207,
                "r2d2-worker-1-861553": 0.0,
                "rocksdb:low1-861507": 0.0,
                "pool-thread-#23-861584": 0.77720207,
                "rocksdb:low9-861515": 0.0,
                "tokio-runtime-w-861545": 0.0,
                "rocksdb:low7-861513": 0.0,
                "pool-thread-#16-861577": 0.0,
                "rocksdb:low6-861512": 0.0,
                "tokio-runtime-w-861539": 0.77720207,
                "tokio-runtime-w-861542": 0.0,
                "pool-thread-#8-861569": 0.0,
                "pool-thread-#20-861581": 0.0,
                "pool-thread-#14-861575": 0.0,
                "pool-thread-#6-861567": 0.0,
                "rocksdb:low0-861506": 0.0,
                "rocksdb:high0-861518": 0.0,
                "pool-thread-#5-861566": 1.5544041,
                "tokio-runtime-w-861543": 0.0,
                "pool-thread-#7-861568": 0.77720207,
                "rocksdb:high1-861519": 0.0,
                "rocksdb:low4-861510": 0.0,
                "pool-thread-#0-861561": 0.77720207,
                "r2d2-worker-1-861550": 0.0
            }
        },
        "validators": {
            "total": 96.373055,
            "validators": {
                "protocol-runner861624": {
                    "collective": 0.0,
                    "taskThreads": {
                        "protocol-runner-861625": 0.0,
                        "ffi-ocaml-execu-861627": 0.0,
                        "ctrl-c-861626": 0.0
                    }
                },
                "protocol-runner861587": {
                    "collective": 96.373055,
                    "taskThreads": {
                        "ffi-ocaml-execu-861590": 95.595856,
                        "ctrl-c-861589": 0.0,
                        "protocol-runner-861588": 0.0,
                        "ffi-ocaml-execu-861629": 0.0
                    }
                }
            }
        }
    },
    "io": {
        "node": {
            "readBytesPerSec": 0,
            "writtenBytesPerSec": 1544192
        },
        "validators": {
            "total": {
                "readBytesPerSec": 0,
                "writtenBytesPerSec": 2084864
            },
            "validators": {
                "protocol-runner-861587": {
                    "readBytesPerSec": 0,
                    "writtenBytesPerSec": 2084864
                },
                "protocol-runner-861624": {
                    "readBytesPerSec": 0,
                    "writtenBytesPerSec": 0
                }
            }
        }
    },
    "network": {
        "sentBytesPerSec": 51766,
        "receivedBytesPerSec": 302240
    }
}
```