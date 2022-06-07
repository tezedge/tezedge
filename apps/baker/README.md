# TezEdge Baker

This is a daemon for creating, signing and injecting endorsements and blocks for the Tezos blockchain, in accordance to the Tenderbake consensus algorithm.

## Running

### Prerequisites

|  OS  |      Versions      |
|----------|:-------------:|
| Ubuntu |  16.04, 18.04, 18.10, 19.04, 19.10, 20.04, 20.10, 21.04, 21.10, 22.04 |
| Debian |  9, 10 |
| OpenSUSE |  15.1, 15.2 |
| CentOS |  8 |
| MacOS |  *experimental* - newer or equal to 10.13 should work, Intel and M1 cpus |

1. Install **Git** (client), **Curl** and **LibSSL**
    ```
    # If you are using ubuntu
    apt install git curl libssl-dev
    ```
2. Install **Rust** command _(We recommend installing Rust through rustup.)_
    ```
    # Run the following in your terminal, then follow the onscreen instructions.
    curl https://sh.rustup.rs -sSf | sh
    source ~/.cargo/env
    ```
3. Install **Rust toolchain** _(Our releases are built with 1.58.1.)_
    ```
    rustup toolchain install 1.58.1
    rustup default 1.58.1
    ```
4. Download `tezos-client` from https://gitlab.com/tezos/tezos/-/releases and put the `tezos-client` binary into one of directories listed in `$PATH` variable
    ```
    sudo mv tezos-client /usr/local/bin
    ```

### Building

#### Option 1: use tool cargo, which is part of **Rust** already installed in previous step

```
cargo install --git https://github.com/tezedge/tezedge --branch develop baker
```

You can use the tag `--tag v2.3.x` instead of branch name `--branch develop`.

```
cargo install --git https://github.com/tezedge/tezedge --tag v2.3.2 develop baker
```

The cargo will create the binary `tezedge-baker` at `~/.cargo/bin`. The directory `~/.cargo/bin` should be in your `$PATH` variable, so you can run `tezedge-baker` without full path. If it is not working (it might not work if you are using a non-standard shell), execute `source ~/.cargo/env` to update the environment.

#### Option 2: clone sources and build

```
git clone https://github.com/tezedge/tezedge --branch develop
cd tezedge
cargo build -p baker --release
```

The binary will be in `target/release`.

Make sure you copy the binary into one of the directories listed in `$PATH` variable, for example:

```
sudo cp target/release/tezedge-baker /usr/local/bin
```

### Prepare an account

Skip this section if you already have a Tezos account ready for baking.

Pick a new <delegate_alias> and generate a new Tezos account using Octez client.

```
tezos-client gen keys <delegate_alias>
```

You need to fund this account with at least 6000 ꜩ. Register the account as delegate and wait a time between 5 and 6 cycles, depending on the position in the cycle (approximately 15 days).

```
tezos-client register key <delegate_alias> as delegate
```

By default, tezos-client storing secret key for the account in `$HOME/.tezos-client` directory.

See the [baking documentation](../../baking/mainnet/README.md#initialize-keys-for-bakerendorser) for more details.

See the [Key management](https://tezos.gitlab.io/user/key-management.html) guide for more information.

### Run the TezEdge node

See [baking documentation](../../baking/mainnet/README.md#run-tezedge-node) for instructions onhow to run the TezEdge node.

### Run the baker

_Note: It is recommended to run the baker with nohup_

Assuming that the secret key (or locator for remote signing) is in `$HOME/.tezos-client` and the node is running locally and uses 18732 port for RPC, the command is:

```
nohup tezedge-baker --base-dir "$HOME/.tezos-client" --endpoint "http://localhost:18732" --baker <delegate_alias> &
```

Additionally, you can run `tezedge-baker --help` to get short help.

Options:

- `--base-dir`: The base directory. The path to the directory where the baker can find secret keys, or the remote signer's location. Usually, it is `~/.tezos-client`. Also, this directory is used by baker as a persistent storage of the state. It is crucial, for example, when revealing the seed nonce in a new cycle.
- `--endpoint`: TezEdge or Tezos node RPC endpoint. Usually the port is `8732` or `18732`. If node is running locally, it will be `http://localhost:8732`.
- `--baker`: The alias of the baker.
- `--archive` or `-a`: If this flag is used, the baker will store verbose information for debug in the base directory.

### Common problems

1. After creating an account and registering it as a delegate, make sure you wait at least 5 cycles (approximately 15 days) before you start to bake.
1. The executable `tezedge-baker` and `tezos-client` should be in the directory which is in the `$PATH` environment variable. It may be `/usr/local/bin`, you need superuser permission to copy in this directory. Also, it is possible to execute `tezos-client` from any directory, but you need to specify the full path, for example `/home/username/tezos/tezos-client gen keys bob`, instead of `tezos-client gen keys bob`.

## Tests

Run from the source code directory:

```
cargo test -p baker
```

### Fuzzing

Install Rust nightly-2021-12-22 and cargo-fuzzcheck from source.

```
rustup install nightly-2021-12-22
cargo +nightly-2021-12-22 install --git https://github.com/tezedge/fuzzcheck-rs cargo-fuzzcheck
```

Run it from the directory `apps/baker`:

```
cargo +nightly-2021-12-22 fuzzcheck --test action_fuzz test_baker
```

### Mitten tests

Prepare Tezos with mitten.

```
git clone https://gitlab.com/nomadic-labs/tezos.git tezos-mitten -b mitten-ithaca
cd tezos-mitten
opam init --disable-sandboxing
make build-deps
eval $(opam env)
make
make mitten
```

Rename the file `tezos-mitten/tezos-baker-012-Psithaca` to `tezos-mitten/tezos-baker-012-Psithaca.octez`

Copy the files `tezedge/apps/baker/tezedge.env` and `tezedge/apps/baker/tezos-baker-012-Psithaca` into the `tezos-mitten` directory.

Build the baker and copy the binary into `tezos-mitten`.

From the `tezedge` directory:
```
cargo build -p baker --release
cp target/release/tezedge-baker ../tezos-mitten/tezos-baker-012-Psithaca.tezedge
```

Now you can run the mitten scenario:

From the `tezos-mitten` directory:
```
dune exec src/mitten/scenarios/no_eqc_stuck.exe
```

You can find more scenarios in `src/mitten/scenarios`.
