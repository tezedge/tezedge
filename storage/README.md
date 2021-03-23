TezEdge storage
==============

1. dbs - operational databases for storing metadata/indexes for block, operations, ...
2. [commit_log](src/commit_log) - contains data for BlockHeader and Operations
3. [sequences](src/persistent/sequence.rs) - sequence generators for ID purposes
4. [context](src/context) module for merkle context key-value store
    - **merkle** - merkle context abstract algorithm + algorithm for context_hash calculation
    - **gc** - support for context garbage collection
    - **actions** - support for context actions recording, when evaluate context
    - **kv_store** - different implementations for key-value stores for merkle algorithm

### Another features

1. Context actions recording [here](docs/action_recording.md)
2. Context actions replaying [here](docs/action_replaying.md)
3. Context garbage collection [here](docs/merkle_storage_gc.md)
