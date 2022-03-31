# P2P Messages

## Handshaking Messages

### ConnectionMessage
Size: up to 214

| Name | Size | Contents |
|:-----|:-----|:---------|
| port | 2 | unsigned 16-bit integer |
| public_key | 32 | fixed-length sequence of bytes |
| proof_of_work_stamp | 24 | fixed-length sequence of bytes |
| message_nonce | 24 | fixed-length sequence of bytes |
| version | up to 132 | [NetworkVersion](#networkversion) |


### MetadataMessage
Size: 2

| Name | Size | Contents |
|:-----|:-----|:---------|
| disable_mempool | 1 | boolean |
| private_node | 1 | boolean |

## Distributed DB Messages

### P2P Disconnect
Size: 6

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x01) |
| message | 0 | empty |


### P2P Bootstrap
Size: 6

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x02) |
| message | 0 | empty |


### P2P Advertise
Size: up to 5106

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x03) |
| message | up to 5100 | [AdvertiseMessage](#advertisemessage) |


### P2P SwapRequest
Size: up to 73

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x04) |
| message | up to 67 | [SwapMessage](#swapmessage) |


### P2P SwapAck
Size: up to 73

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x05) |
| message | up to 67 | [SwapMessage](#swapmessage) |


### P2P GetCurrentBranch
Size: 10

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x10) |
| message | 4 | [GetCurrentBranchMessage](#getcurrentbranchmessage) |


### P2P CurrentBranch
Size: up to 8420622

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x11) |
| message | up to 8420616 | [CurrentBranchMessage](#currentbranchmessage) |


### P2P Deactivate
Size: 10

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x12) |
| message | 4 | [DeactivateMessage](#deactivatemessage) |


### P2P GetCurrentHead
Size: 10

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x13) |
| message | 4 | [GetCurrentHeadMessage](#getcurrentheadmessage) |


### P2P CurrentHead
Size: up to 8516634

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x14) |
| message | up to 8516628 | [CurrentHeadMessage](#currentheadmessage) |


### P2P GetBlockHeaders
Size: up to 330

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x20) |
| message | up to 324 | [GetBlockHeadersMessage](#getblockheadersmessage) |


### P2P BlockHeader
Size: up to 8388614

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x21) |
| message | up to 8388608 | [BlockHeaderMessage](#blockheadermessage) |


### P2P GetOperations
Size: up to 330

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x30) |
| message | up to 324 | [GetOperationsMessage](#getoperationsmessage) |


### P2P Operation
Size: up to 131110

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x31) |
| message | up to 131104 | [OperationMessage](#operationmessage) |


### P2P GetProtocols
Size: up to 330

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x40) |
| message | up to 324 | [GetProtocolsMessage](#getprotocolsmessage) |


### P2P Protocol
Size: up to 2097158

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x41) |
| message | up to 2097152 | [ProtocolMessage](#protocolmessage) |


### P2P GetOperationsForBlocks
Size: up to 340

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x60) |
| message | up to 334 | [GetOperationsForBlocksMessage](#getoperationsforblocksmessage) |


### P2P OperationsForBlocks
Size: up to 1048715

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the message | 4 | unsigned 32-bit integer |
| tag | 2 | tag corresponding to the message (0x61) |
| message | up to 1048709 | [OperationsForBlocksMessage](#operationsforblocksmessage) |

## Auxiliary Types

### AdvertiseMessage
Size: up to 5100

| Name | Size | Contents |
|:-----|:-----|:---------|
| id | up to 5100 | up to 100 of UTF8-encoded string |


### BlockHeader
Size: variable

| Name | Size | Contents |
|:-----|:-----|:---------|
| level | 4 | signed 32-bit integer |
| proto | 1 | unsigned byte |
| predecessor | 32 | BlockHash |
| timestamp | 8 | timestamp |
| validation_pass | 1 | unsigned byte |
| operations_hash | 32 | OperationListListHash |
| # of bytes in the next field (up to 96) | 4 | unsigned 32-bit integer |
| fitness | variable | list of [BlockHeader.fitness items](#blockheaderfitness-items) |
| context | 32 | ContextHash |
| protocol_data | up to 8388398 | list of unsigned byte |


### BlockHeader.fitness items
Size: variable

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| item | variable | list of unsigned byte |


### BlockHeaderMessage
Size: up to 8388608

| Name | Size | Contents |
|:-----|:-----|:---------|
| block_header | up to 8388608 | [BlockHeader](#blockheader) |


### Component
Size: up to 205825

| Name | Size | Contents |
|:-----|:-----|:---------|
| name | up to 1024 | UTF8-encoded string with 1024 elements max |
| presense of the next field | 1 | 0xff if presend, 0x00 if absent  |
| interface | up to 102400 | UTF8-encoded string with 102400 elements max |
| implementation | up to 102400 | UTF8-encoded string with 102400 elements max |


### CurrentBranch
Size: up to 8420612

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field (up to 8388608) | 4 | unsigned 32-bit integer |
| current_head | up to 8388608 | [BlockHeader](#blockheader) |
| history | up to 32000 | up to 1000 of BlockHash |


### CurrentBranchMessage
Size: up to 8420616

| Name | Size | Contents |
|:-----|:-----|:---------|
| chain_id | 4 | ChainId |
| current_branch | up to 8420612 | [CurrentBranch](#currentbranch) |


### CurrentHeadMessage
Size: up to 8516628

| Name | Size | Contents |
|:-----|:-----|:---------|
| chain_id | 4 | ChainId |
| # of bytes in the next field (up to 8388608) | 4 | unsigned 32-bit integer |
| current_block_header | up to 8388608 | [BlockHeader](#blockheader) |
| current_mempool | up to 128012 | [Mempool](#mempool) |


### DeactivateMessage
Size: 4

| Name | Size | Contents |
|:-----|:-----|:---------|
| deactivate | 4 | ChainId |


### GetBlockHeadersMessage
Size: up to 324

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| get_block_headers | up to 320 | up to 10 of BlockHash |


### GetCurrentBranchMessage
Size: 4

| Name | Size | Contents |
|:-----|:-----|:---------|
| chain_id | 4 | ChainId |


### GetCurrentHeadMessage
Size: 4

| Name | Size | Contents |
|:-----|:-----|:---------|
| chain_id | 4 | ChainId |


### GetOperationsForBlocksMessage
Size: up to 334

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| get_operations_for_blocks | up to 330 | up to 10 of [OperationsForBlock](#operationsforblock) |


### GetOperationsMessage
Size: up to 324

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| get_operations | up to 320 | up to 10 of OperationHash |


### GetProtocolsMessage
Size: up to 324

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| get_protocols | up to 320 | up to 10 of ProtocolHash |


### Mempool
Size: up to 256012

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| known_valid | up to 128000 | up to 4000 of OperationHash |
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| pending | up to 128000 | up to 4000 of OperationHash |


### NetworkVersion
Size: up to 132

| Name | Size | Contents |
|:-----|:-----|:---------|
| chain_name | up to 128 | UTF8-encoded string with 128 elements max |
| distributed_db_version | 2 | unsigned 16-bit integer |
| p2p_version | 2 | unsigned 16-bit integer |


### Operation
Size: up to 131104

| Name | Size | Contents |
|:-----|:-----|:---------|
| branch | 32 | BlockHash |
| data | up to 131072 | list of unsigned byte |


### OperationMessage
Size: up to 131104

| Name | Size | Contents |
|:-----|:-----|:---------|
| operation | up to 131104 | [Operation](#operation) |


### OperationsForBlock
Size: 33

| Name | Size | Contents |
|:-----|:-----|:---------|
| hash | 32 | BlockHash |
| validation_pass | 1 | signed byte |


### OperationsForBlocksMessage
Size: up to 1048709

| Name | Size | Contents |
|:-----|:-----|:---------|
| operations_for_block | 33 | [OperationsForBlock](#operationsforblock) |
| operation_hashes_path | up to 100 | Merkle tree path encoding |
| operations | up to 1048576 | list of [OperationsForBlocksMessage.operations items](#operationsforblocksmessageoperations-items) |


### OperationsForBlocksMessage.operations items
Size: up to 131108

| Name | Size | Contents |
|:-----|:-----|:---------|
| # of bytes in the next field | 4 | unsigned 32-bit integer |
| item | up to 131104 | [Operation](#operation) |


### Protocol
Size: variable

| Name | Size | Contents |
|:-----|:-----|:---------|
| expected_env_version | 2 | signed 16-bit integer |
| # of bytes in the next field (up to 2097146) | 4 | unsigned 32-bit integer |
| components | variable | list of [Component](#component) |


### ProtocolMessage
Size: up to 2097152

| Name | Size | Contents |
|:-----|:-----|:---------|
| protocol | up to 2097152 | [Protocol](#protocol) |


### SwapMessage
Size: up to 67

| Name | Size | Contents |
|:-----|:-----|:---------|
| point | up to 51 | UTF8-encoded string |
| peer_id | 16 | CryptoboxPublicKeyHash |

