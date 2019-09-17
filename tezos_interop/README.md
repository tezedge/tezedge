Tezos interoperability
==============

You can setup how code in this package is built and linked by setting corresponding environment variables.

### Compiling OCaml code
`OCAML_BUILD_CHAIN` is used to specify which build chain will be used to compile ocaml code.
Default value is `remote`.

Valid values are:
* `docker` is used to build shared library from ocaml code in docker container. This is here as a convenience option for
people who don't want to install OCaml. First run might take some time to complete because docker images are fairly large. 
* `local` use this option if you have OCaml already installed.
* `remote` is used when precompiled linux binary should be used.

Note: the first time you build using docker might take a long time because it's building ocaml image from scratch.
In the future we will shorten this time by providing a prebuild docker image.
