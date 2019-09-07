Tezos interoperability
==============

You can setup how code in this package is built and linked by setting corresponding environment variables.

### Compiling OCaml code
`OCAML_BUILD_CHAIN` is used to specify which build chain will be used to compile ocaml code.
Default value is `docker`.

Valid values are:
* `docker` is used to build shared library from ocaml code in docker container. This is here as a convenience option for
people who don't want to install OCaml. First run might take some time to complete because docker images are fairly large. 
* `local` use this option if you have OCaml already installed.

Note: the first time you build using docker might take a long time because it's building ocaml image from scratch.
In the future we will shorten this time by providing a prebuild docker image.

### Linking OCaml libraries
`OCAML_LIB` environment variable will set name of the ocaml library which will be used fer linking.
Default value is `tzmock`.

Valid values are:
* `tzmock` will use mock ocaml code, at the moment this is the only valid option
* `tzreal` will build forked version of the Tezos repository.
  * `UPDATE_GIT_SUBMODULES` (default `true`) is used to skip git update of Tezos repository.
  * `TEZOS_BASE_DIR` is used to change location of Tezos repository on file system.

### Running Ocaml runtime
`STORAGE_DATA_DIR` environment variable is used to specify directory where tezos storage stores data (contex/store). 