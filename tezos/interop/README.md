Tezos interoperability
==============

You can setup how code in this package is built and linked by setting corresponding environment variables.

### Compiling OCaml code
`OCAML_BUILD_CHAIN` is used to specify which build chain will be used to compile ocaml code.
Default value is `remote`.

Valid values are:
* `local` use this option if you have OCaml already installed and you want to build Ocaml FFI library from the Tezos sources
  * `build.rs` see `GIT_REPO_URL` points to Tezos repository
  * `build.rs` see `GIT_COMMIT_HASH` points to last supported release
  * `UPDATE_GIT` (default `true`) is used to skip git update of Tezos repository.
  * `TEZOS_BASE_DIR` (defalt `src`) is used to change location of Tezos repository on the file system for Makefile.
* `remote` is used when precompiled linux binary should be used.
  * For list of supported platform visit [releases](https://gitlab.com/simplestaking/tezos/-/releases) page.
  * Last supported release with distribuitions are configured in `build.rs` as `GIT_RELEASE_DISTRIBUTIONS_URL` which points to `libtezos-ffi-distribution-summary.json`
  * build automatically resolves OS a downloads correct pre-build library distribution

##### Local OCaml development
If you want to build rust library with local build of Ocaml library from Tezos sources, you can set env variables to:
* `OCAML_BUILD_CHAIN=local`
* `UPDATE_GIT=false`
* `TEZOS_BASE_DIR=<your-local-directory-with-tezos-sources>`

and run `SODIUM_USE_PKG_CONFIG=1 cargo build` to build tezedge node manually.

### Run tests and benches
```
cargo test
```
```
cargo bench
```
```
cargo bench --tests
```