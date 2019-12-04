# Build stage 1
FROM simplestakingcom/tezos-opam-builder:debian10
# Checkout and compile tezedge source code
ARG tezedge_git="https://github.com/simplestaking/tezedge.git"
ARG rust_toolchain="nightly-2019-11-25"
RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain ${rust_toolchain} -y
ENV PATH=/home/appuser/.cargo/bin:$PATH
ENV RUST_BACKTRACE=1
ENV SODIUM_USE_PKG_CONFIG=1
ENV OCAML_BUILD_CHAIN=remote
RUN cd /home/appuser && git clone ${tezedge_git} && cd tezedge && git checkout master
WORKDIR /home/appuser/tezedge
ENTRYPOINT ["./run.sh", "release"]
CMD []
