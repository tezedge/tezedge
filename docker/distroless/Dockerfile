FROM simplestakingcom/tezos-opam-builder:debian10 as build-env

# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/simplestaking/tezedge.git"
ARG rust_toolchain="nightly-2020-10-24"
ARG SOURCE_BRANCH
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
RUN cd /home/appuser && git clone ${tezedge_git} --branch ${SOURCE_BRANCH} && cd tezedge && cargo build --release #5
WORKDIR /home/appuser/tezedge
RUN mkdir /tmp/tezedge
RUN mkdir /tmp/tezedge/light-node
RUN chown appuser:appuser /tmp/tezedge

FROM gcr.io/distroless/cc-debian10 as light-node
COPY --from=build-env /tmp/tezedge /

# Copy binaries
COPY --from=build-env /home/appuser/tezedge/target/release/light-node /
COPY --from=build-env /home/appuser/tezedge/target/release/protocol-runner /
COPY --from=build-env /home/appuser/tezedge/target/release/sandbox /
COPY --from=build-env /home/appuser/tezedge/sandbox/artifacts/tezos-client /

COPY --from=build-env /home/appuser/tezedge/docker/distroless/tezedge.config /

# Copy shared libraries
COPY --from=build-env /home/appuser/tezedge/tezos/interop/lib_tezos/artifacts/libtezos.so /
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libsodium.so.23 /usr/lib/x86_64-linux-gnu/libsodium.so.23
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libev.so.4 /usr/lib/x86_64-linux-gnu/libev.so.4
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libgmp.so.10 /usr/lib/x86_64-linux-gnu/libgmp.so.10
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libffi.so.6 /usr/lib/x86_64-linux-gnu/libffi.so.6

ENV LD_LIBRARY_PATH="/"
CMD [ "/light-node", "--config-file", "/tezedge.config"]

FROM light-node as sandbox

ENV LD_LIBRARY_PATH="/"
# Add aditional libs used by the sandbox module
# Libs required by tezos-client
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libhidapi-libusb.so.0 /usr/lib/x86_64-linux-gnu/libhidapi-libusb.so.0
COPY --from=build-env /lib/x86_64-linux-gnu/libusb-1.0.so.0 /lib/x86_64-linux-gnu/libusb-1.0.so.0
COPY --from=build-env /lib/x86_64-linux-gnu/libudev.so.1 /lib/x86_64-linux-gnu/libudev.so.1

ENV TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER="Y"
CMD [ "/sandbox", "--log-level", "debug", "--sandbox-rpc-port", "3030", "--light-node-path", "/light-node", "--protocol-runner-path", "/protocol-runner", "--tezos-client-path", "/tezos-client"]
