# Build stage 1
FROM simplestakingcom/tezos-opam-builder:debian10
ARG rust_toolchain="nightly-2020-10-24"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote

# Copies actual source codes directory to docker
COPY --chown=appuser:appuser . /home/appuser/tezedge

WORKDIR /home/appuser/tezedge
ENTRYPOINT ["./run.sh", "release"]
CMD []
