# Telemetry, Metrics and Statistics 

This project contains implementation of `Monitors` and `Handlers` 
for generating and emitting statistical data about running node.

# API
Detailed information about outgoing and (in future) incoming messages.

## WebSocket
Outgoing messages sent by websocket handler are always encoded in 
standard JSON format, having structure consisting of two attributes:
- `type` attribute, name of the type of the message.
- `payload` attribute, content of the message.

### Peers
#### Status messages
Node emits messages about incoming and outgoing connections to other nodes.
Those messages are emitted ad-hoc. Example: 
```
{
    "type": "peerStatus",
    "payload": {
      "status": "connected",
      "id": "34.255.45.196:9732"
    }
}
```
where
- `status` might be either `connected` or `disconnected`.
- `id` is unique peer identification, at the moment, peer is identifier by its IP address.

#### Metrics messages
Node periodically emits messages about individual peers transfer metrics.
For every peer block in `payload`, there should previously come an `peerStatus:connected` message beforehand. 
Example:
```
{
    "type": "peersMetrics",
    "payload": [{
        "id": "idse7w6uFyRyaM2DLVwLs4hZ6XNKL6",
        "ipAddress": "34.255.45.196:9732",
        "transferredBytes": 651128,
        "averageTransferSpeed": 3003.3674,
        "currentTransferSpeed": 3467.233 
    }, {
        "id": null,
        "ipAddress": "[2a05:d018:791:4602:6e77:2055:676d:c75c]:9732",
        "transferredBytes": 0,
        "averageTransferSpeed": 0,
        "currentTransferSpeed": 0
    }]
}
```
Where:
- `id` is an public key of a peer. This attribute might not be present before proper handshake.
- `ipAddress` is address of a peer. This value should be always present.
- `transferredBytes` is total bytes transferred by this client for whole session.
- `averageTransferSpeed` is calculated for the whole session in bytes/seconds.
- `currentTransferSpeed` is calculated for last second in bytes/seconds.

### Progress
#### Incoming Transfer
Node periodically emits messages about incoming transfer and bootstrap statistics. 
```
{
    "type": "incomingTransfer",
    "payload": {
        "eta": 8054.818,
        "currentBlockCount": 695156,
        "downloadedBlocks": 83009,
        "downloadRate": 84.29936,
        "averageDownloadRate": 73.937744
    }
}
```
Where
- `eta` is rough remaining time to download all blocks with operations in seconds.
- `currentBlockCount` - number of blocks in the block-chain.
- `downloadedBlocks` - number of downloaded blocks by this node.
- `downloadRate` - rate in last second, at which this node downloads blocks with its operations in block/seconds
- `averageDownloadRate`  - rate for whole session, at which this node downloads blocks with its operations in bloc/seconds.


#### Blocks Status
Node periodically emits messages, about detail blocks downloading progress, in blocks groups. Each group contains `4096`
blocks. 
```
{
    "type": "blockStatus",
    "payload": [{
        "group": 0,
        "numbersOfBlocks": 4096,
        "finishedBlocks": 4096,
        "appliedBlocks": 0,
        "downloadDuration": 39.20117
    }, {
        "group": 1,
        "numbersOfBlocks": 1863,
        "finishedBlocks": 1003,
        "appliedBlocks": 0,
        "downloadDuration": null
    }]
}
```
Where:
- `group` is group identification
- `numberOfBlocks` is number of assigned blocks, from their blocks headers.
- `finishedBlocks` is number of blocks with already downloaded operations.
- `downloadDuration` is number of seconds, which took to finish downloading this whole block group, if group is not finished,
then value should be `null`