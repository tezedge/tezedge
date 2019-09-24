Tezos interoperability
==============

You can setup how code in this package is built and linked by setting corresponding environment variables.

### Compiling OCaml code
`OCAML_BUILD_CHAIN` is used to specify which build chain will be used to compile ocaml code.
Default value is `local`.

Valid values are:
* `local` use this option if you have OCaml already installed.
* `remote` is used when precompiled linux binary should be used (experimental).

##### Local OCaml development
* `UPDATE_GIT` (default `true`) is used to skip git update of Tezos repository.
* `TEZOS_BASE_DIR` (defalt `src`) is used to change location of Tezos repository on the file system for Makefile.

### Running Ocaml runtime
`OCAML_LOG_ENABLED` (default false) environment variable which turn on/off logging in Tezos OCaml runtime.
