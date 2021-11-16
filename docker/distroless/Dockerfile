ARG BASE_IMAGE=tezedge/tezedge-libs:latest

FROM tezedge/tezos-opam-builder:debian10 as build-env

USER root
RUN apt-get update && apt-get install -y libssl-dev zlib1g

# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/tezedge/tezedge.git"
ARG rust_toolchain="nightly-2021-08-04"
ARG SOURCE_BRANCH
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
ARG RUSTFLAGS=""
ENV OCAML_WHERE_PATH=/tmp/ocaml-includes
RUN wget https://gitlab.com/tezedge/tezos/uploads/1bf4d1d130e75129be82a148149b04b3/libtezos-ffi-headers.tar.gz
RUN mkdir -p $OCAML_WHERE_PATH; tar xvzf libtezos-ffi-headers.tar.gz -C $OCAML_WHERE_PATH
RUN cd /home/appuser && git clone ${tezedge_git} --branch ${SOURCE_BRANCH} && cd tezedge && RUSTFLAGS=${RUSTFLAGS} cargo build --release #5

WORKDIR /home/appuser/tezedge
RUN mkdir /tmp/tezedge
RUN mkdir /tmp/tezedge/light-node
RUN chown appuser:appuser /tmp/tezedge

FROM ${BASE_IMAGE} as light-node
COPY --from=build-env /tmp/tezedge /

# Copy binaries
COPY --from=build-env /home/appuser/tezedge/target/release/light-node /
COPY --from=build-env /home/appuser/tezedge/target/release/protocol-runner /
COPY --from=build-env /home/appuser/tezedge/target/release/sandbox /
COPY --from=build-env /home/appuser/tezedge/sandbox/artifacts/tezos-client /

COPY --from=build-env /home/appuser/tezedge/docker/distroless/tezedge.config /

# Copy shared libraries
COPY --from=build-env /home/appuser/tezedge/tezos/sys/lib_tezos/artifacts/libtezos.so /usr/lib/x86_64-linux-gnu/libtezos.so
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libev.so.4 /usr/lib/x86_64-linux-gnu/libev.so.4
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libffi.so.6 /usr/lib/x86_64-linux-gnu/libffi.so.6
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libzstd.so.1 /usr/lib/x86_64-linux-gnu/libz.so.1
COPY --from=build-env /lib/x86_64-linux-gnu/libnss_dns.so.2 /lib/x86_64-linux-gnu/libnss_dns.so.2
COPY --from=build-env /lib/x86_64-linux-gnu/libnss_files.so.2 /usr/lib/x86_64-linux-gnu/libnss_files.so
COPY --from=build-env /lib/x86_64-linux-gnu/libresolv.so.2 /lib/x86_64-linux-gnu/libresolv.so.2

# Copy zcash-params init files for sapling
COPY --from=build-env /home/appuser/tezedge/tezos/sys/lib_tezos/artifacts/sapling-spend.params /
COPY --from=build-env /home/appuser/tezedge/tezos/sys/lib_tezos/artifacts/sapling-output.params /

# Default entry point runs node with default config + several default values, which can be overriden by CMD
ENTRYPOINT [ "/light-node", "--config-file=/tezedge.config", "--p2p-port=9732", "--rpc-port=18732", "--init-sapling-spend-params-file=/sapling-spend.params", "--init-sapling-output-params-file=/sapling-output.params"]

FROM light-node as sandbox

# Add aditional libs used by the sandbox module
# Libs required by tezos-client
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libhidapi-libusb.so.0 /usr/lib/x86_64-linux-gnu/libhidapi-libusb.so.0
COPY --from=build-env /lib/x86_64-linux-gnu/libusb-1.0.so.0 /lib/x86_64-linux-gnu/libusb-1.0.so.0
COPY --from=build-env /lib/x86_64-linux-gnu/libudev.so.1 /lib/x86_64-linux-gnu/libudev.so.1

ENV TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER="Y"
# Default entry point runs sandbox launcher with default config + several default values, which can be overriden by CMD
ENTRYPOINT [ "/sandbox", "--light-node-path=/light-node", "--protocol-runner-path=/protocol-runner", "--tezos-client-path=/tezos-client", "--log-level=debug", "--sandbox-rpc-port=3030", "--init-sapling-spend-params-file=/sapling-spend.params", "--init-sapling-output-params-file=/sapling-output.params"]