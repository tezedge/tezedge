# ARG BASE_IMAGE=tezedge/tezedge-libs:latest

# FROM tezedge/tezos-opam-builder:debian10 as build-env

FROM debian:10 as build-env
USER root
RUN apt-get update && apt-get install -y libssl-dev pkg-config libsodium-dev git curl

# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/tezedge/tezedge.git"
ARG rust_toolchain="1.58.1"
ARG SOURCE_BRANCH
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/root/.cargo/bin:$PATH
ENV SODIUM_USE_PKG_CONFIG=1
RUN apt-get install -y clang libclang-dev zlib1g
RUN git clone ${tezedge_git} --branch ${SOURCE_BRANCH} && cd tezedge && cd apps/node_monitoring && cargo build --release #9

# WORKDIR /home/appuser/tezedge

FROM debian:10

USER root
RUN apt-get update && apt-get install -y libssl-dev curl
# Copy binaries
COPY --from=build-env /tezedge/apps/node_monitoring/target/release/node-monitoring /node-monitoring

# Copy shared libraries
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libssl.so.1.1 /usr/lib/x86_64-linux-gnu/libssl.so.1.1
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libcrypto.so.1.1 /usr/lib/x86_64-linux-gnu/libcrypto.so.1.1
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libzstd.so.1 /usr/lib/x86_64-linux-gnu/libz.so.1
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libsodium.so.23 /usr/lib/x86_64-linux-gnu/libsodium.so.23
COPY --from=build-env /lib/x86_64-linux-gnu/libc.so.6 /lib/x86_64-linux-gnu/libc.so.6

# Default entry point runs monitoring with default config + several default values, which can be overriden by CMD
ENTRYPOINT [ "/node-monitoring" ]
