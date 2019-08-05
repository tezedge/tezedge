Integration tests
=================

This directory contains set of integration tests.

### `tezos_interop`
This will test interface designed to communicate with tezos code base.

Ocaml code is stored in `lib_ocaml` directory where is compiled to a shared library.
Compiled library is then used by tests to verify that interfaces are correct. 