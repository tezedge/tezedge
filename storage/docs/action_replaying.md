# Action Replaying

## Get an actions file

**Two possibilities:**
- see action recording documentation [here](action_recording.md)
- or use dedicated test to generate it
```
# build project
cargo build --release

# run test to generate action file
TARGET_ACTION_FILE=/tmp/test_action_file.data PROTOCOL_RUNNER=../target/release/protocol-runner cargo test --release -- --nocapture --ignored test_process_bootstrap_level1324_and_generate_action_file
```

Here we should have stored a new action file according to `TARGET_ACTION_FILE=/tmp/test_action_file.data`

