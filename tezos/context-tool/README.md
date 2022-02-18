# Tezedge context tool

Binary tool for the persistent context

## Supported commands:

- `build-integrity`: Build a valid `sizes.db` based on a context
- `is-valid-context`: Check if the context is valid, based on the file `sizes.db`.  
  It will computes the checksum of the files and verify their sizes.
- `context-size`: Inspect the context of a commit and display how many bytes occupy its objects
- `make-snapshot`: Create a snapshot based on a single commit.  
  This will create a new context with all unused objects removed.
- `dump-checksums`: Display `sizes.db` in an human readable format 


## Example commands:

- `cargo run --bin context-tool -- build-integrity -c PATH_TO_CONTEXT`
- `cargo run --bin context-tool -- is-valid-context -c PATH_TO_CONTEXT`
- `cargo run --bin context-tool -- context-size -c PATH_TO_CONTEXT)`
- `cargo run --bin context-tool -- make-snapshot -c PATH_TO_CONTEXT`
- `cargo run --bin context-tool -- dump-checksums -c PATH_TO_CONTEXT`
