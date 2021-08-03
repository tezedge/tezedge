ARG BASE_IMAGE=tezedge/tezedge-libs:latest

FROM tezedge/tezos-opam-builder:debian10 as build-env

USER root
RUN apt-get update && apt-get install -y libssl-dev

# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/mambisi/tezedge.git"
ARG rust_toolchain="nightly-2020-12-31"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
ARG RUSTFLAGS=""
ENV OCAML_WHERE_PATH=/tmp/ocaml-includes
RUN wget https://gitlab.com/tezedge/tezos/uploads/1bf4d1d130e75129be82a148149b04b3/libtezos-ffi-headers.tar.gz
RUN mkdir -p $OCAML_WHERE_PATH; tar xvzf libtezos-ffi-headers.tar.gz -C $OCAML_WHERE_PATH
RUN cd /home/appuser && git clone ${tezedge_git} --branch storage-latency-temp && cd tezedge && RUSTFLAGS=${RUSTFLAGS} cargo run --bin storage-metrics --release #5
