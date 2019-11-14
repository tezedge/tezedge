# Build stage 1
FROM simplestakingcom/tezos-opam-builder:debian10
# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/simplestaking/tezedge.git"
ARG tezedge_commit_hash="11b3b5ed26efbecd0a62635c009338461aad3f56"
ARG rust_toolchain="nightly-2019-10-14"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
RUN cd /home/appuser && \
	git clone ${tezedge_git} && \
    cd tezedge && \
    git checkout ${tezedge_commit_hash} && \
    cargo build

# Build stage 2
FROM debian:10
COPY --from=0 /home/appuser/tezedge/target/debug/light-node /bin/light-node
COPY --from=0 /home/appuser/tezedge/target/debug/protocol-runner /bin/protocol-runner
COPY --from=0 /home/appuser/tezedge/tezos/interop/lib_tezos/artifacts/libtezos.o /lib/libtezos.o
COPY --from=0 /home/appuser/tezedge/light_node/config /config
RUN apt-get update && \
    apt-get install -y libgmp-dev libev-dev libsodium23