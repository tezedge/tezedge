# TezEdge Baker

The daemon for create, sign and inject endorsements and blocks for Tezos blockchain according to Tenderbake consensus algorithm.

## Prerequisites

Linux or MacOS.

## Building

Use cargo:

```
cargo install --git https://github.com/tezedge/tezedge --branch develop baker
```

Also, you can use tag `--tag v2.x.x` instead of branch name `--branch develop`.

The cargo will create the binary `baker` at `~/.cargo/bin`.

The building from source is also trivial.

```
git clone https://github.com/tezedge/tezedge --branch develop
cd tezedge
cargo build -p baker --release
```

The binary will be in `target/release`.

## Running

### Options

- `--base-dir`: The base directory. Path to directory where baker can found secret keys, or remote signer location. Usually it is `~/.tezos-client`. Also this directory is used by baker as a persistent storage of the state. It is crucial, for example, for revealing seed nonce in the new cycle.
- `--endpoint`: TezEdge or Tezos node RPC endpoint. Usually the port is `8732`. If node is running locally, it will be `http://localhost:8732`.
- `--baker`: The alias of the baker.
- `--archive` or `-a`: If this flag is used, the baker will store verbose information for debug in the base directory.

Run `baker --help` to get short help.

## Tests

Run from source code directory:

```
cargo test -p baker
```

### Fuzzing

Install Rust nightly-2021-12-22 and cargo-fuzzcheck from source.

```
rustup install nightly-2021-12-22
cargo +nightly-2021-12-22 install --git https://github.com/tezedge/fuzzcheck-rs cargo-fuzzcheck
```

Run from directory `apps/baker`:

```
cargo +nightly-2021-12-22 fuzzcheck --test action_fuzz test_baker
```

### Mitten tests

Prepare tezos with mitten.

```
git clone https://gitlab.com/nomadic-labs/tezos.git tezos-mitten -b mitten-ithaca
cd tezos-mitten
opam init --disable-sandboxing
make build-deps
eval $(opam env)
make
make mitten
```

Rename file `tezos-mitten/tezos-baker-012-Psithaca` -> `tezos-mitten/tezos-baker-012-Psithaca.octez`

Copy files `tezedge/apps/baker/tezedge.env` and `tezedge/apps/baker/tezos-baker-012-Psithaca` into `tezos-mitten` directory.

Build the baker and copy the binary into `tezos-mitten`.

From `tezedge` directory:
```
cargo build -p baker --release
cp target/release/baker ../tezos-mitten/tezos-baker-012-Psithaca.tezedge
```

Now you can run mitten scenario:

From `tezos-mitten` directory:
```
dune exec src/mitten/scenarios/no_eqc_stuck.exe
```

Find more scenarios in `src/mitten/scenarios`.
