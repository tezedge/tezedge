# Build stage 1
FROM tezedge/tezos-opam-builder:debian10

USER root
RUN apt-get install -y openssl libssl-dev zlib1g

USER appuser

ARG rust_toolchain="1.57"
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
