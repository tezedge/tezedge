Tezos interoperability
==============

You can setup how the code in this package is built and linked by setting corresponding environment variables.

### Compiling OCaml code (optional)

By default, precompiled `libtezos-ffi` binaries will be downloaded when building this library:

* For list of supported platform visit [releases](https://gitlab.com/tezedge/tezos/-/releases) page.
* Last supported release with distribuitions are configured in `build.rs` and specified in `libtezos-ffi-distribution-summary.json`
* The build script automatically detects the host operating system and downloads the correct pre-built `tezos-ffi` library.

But if you want to build this library using a local build of the `libtezos-ffi` OCaml library from custom Tezos sources, you can set the `TEZOS_BASE_DIR` environment variable:

```
TEZOS_BASE_DIR=<your-local-directory-with-tezos-sources>
```

and run `SODIUM_USE_PKG_CONFIG=1 cargo build` to build tezedge node manually.

Note that the build script will not try to build `libtezos-ffi` on it's own. To do so run:

```
git clone https://gitlab.com/tezedge/tezos.git $TEZOS_BASE_DIR
cd $TEZOS_BASE_DIR
env OPAMYES=1 make build-dev-deps
opam config exec -- make
```

**Prerequisistes**: libev, opam

If using Linux, install according to the distribution requirements.

On macOS, using [Homebrew](https://brew.sh/):

```
brew install opam libev
```

### Run tests and benches

On macOS, first set the path to the libtezos library:

```
export DYLD_LIBRARY_PATH=`(pwd)`/tezos/sys/lib_tezos/artifacts
```

Then:

```
cargo test
```
```
cargo bench
```
```
cargo bench --tests
```