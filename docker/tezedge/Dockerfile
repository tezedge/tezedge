# Build stage 1
FROM simplestakingcom/tezos-opam-builder:debian10
# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/simplestaking/tezedge.git"
ARG rust_toolchain="nightly-2019-10-14"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
RUN cd /home/appuser && \
	git clone ${tezedge_git} && \
    cd tezedge && \
    cargo build

# Build stage 2
FROM debian:10
COPY --from=0 /home/appuser/tezedge/target/debug/light-node /bin/light-node
COPY --from=0 /home/appuser/tezedge/target/debug/protocol-runner /bin/protocol-runner
COPY --from=0 /home/appuser/tezedge/tezos/interop/lib_tezos/artifacts/libtezos.so /lib/libtezos.so
COPY --from=0 /home/appuser/tezedge/target/debug/libtezos_interop_callback.so /lib/libtezos_interop_callback.so
COPY --from=0 /home/appuser/tezedge/light_node/config /config
RUN apt-get update && \
    apt-get install -y libgmp-dev libev-dev libsodium23