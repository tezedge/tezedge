Fuzz testing
============

This directory contains fuzz targets. Each sub-directory is a separate fuzz target. 

Prerequisites:
```
sudo apt install build-essential binutils-dev libunwind-dev libblocksruntime-dev
cargo install honggfuzz
```

How to run fuzz target:

If you want to for example want to run `fuzz_connection_message` fuzz target, then execute the following command:
```
cargo hfuzz run fuzz_connection_message
```

In case fuzzing yields a crash, then you can debug it by running:
```
cargo hfuzz run-debug fuzz_connection_message hfuzz_workspace/fuzz_connection_message/*.fuzz
```
