# Build stage 1
FROM simplestakingcom/tezos-opam-builder:debian10 as build-env

# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/simplestaking/tezedge.git"
ARG rust_toolchain="nightly-2020-07-12"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
# TODO: when merged into develop we should use --branch develop and when its released (merged into master) remove the --branch option completely
RUN cd /home/appuser && git clone ${tezedge_git} --branch develop && cd tezedge && cargo build --release
WORKDIR /home/appuser/tezedge
RUN mkdir /tmp/tezedge
RUN mkdir /tmp/tezedge/light-node
RUN chown appuser:appuser /tmp/tezedge

FROM gcr.io/distroless/cc-debian10
COPY --from=build-env /tmp/tezedge /

# Copy binaries
COPY --from=build-env /home/appuser/tezedge/target/release/light-node /
COPY --from=build-env /home/appuser/tezedge/target/release/protocol-runner /
COPY --from=build-env /home/appuser/tezedge/target/release/sandbox /
COPY --from=build-env /home/appuser/tezedge/sandbox/artifacts/tezos-client /
COPY --from=build-env /home/appuser/tezedge/light_node/etc/tezedge_sandbox/sandbox-patch-context.json /
COPY --from=build-env /home/appuser/tezedge/docker/distroless/tezedge.config /

# Copy shared libraries
COPY --from=build-env /home/appuser/tezedge/tezos/interop/lib_tezos/artifacts/libtezos.so /
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libsodium.so.23 /usr/lib/x86_64-linux-gnu/libsodium.so.23
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libev.so.4 /usr/lib/x86_64-linux-gnu/libev.so.4
COPY --from=build-env /usr/lib/x86_64-linux-gnu/libgmp.so.10 /usr/lib/x86_64-linux-gnu/libgmp.so.10

ENV LD_LIBRARY_PATH="/"
CMD [ "/light-node", "--config-file", "/tezedge.config"]