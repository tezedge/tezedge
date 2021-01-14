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
# sanitizer settings
ENV DUMMMMMM="DAMMM"
ENV SANITIZE_ARGS="-Zbuild-std --target=x86_64-unknown-linux-gnu"
ENV TARGET_PATH="target/x86_64-unknown-linux-gnu/debug"
ENV RUSTFLAGS="-Zsanitizer=address"
ENV ASAN_OPTIONS="suppressions=asan.supp"

COPY . /home/appuser/tezedge

USER root
RUN chown -R appuser:appuser /home/appuser/tezedge
RUN apt-get update && apt-get install clang libclang-dev libssl-dev -y
RUN echo "interceptor_via_lib:protocol-runner" > /home/appuser/tezedge/asan.supp
USER appuser

RUN rustup component add rust-src

RUN cd /home/appuser && \
    # git clone ${tezedge_git} --branch ${SOURCE_BRANCH} && \
    cd tezedge && \
    # echo "interceptor_via_lib:protocol-runner" > asan.supp && \
    cargo build ${SANITIZE_ARGS}
WORKDIR /home/appuser/tezedge

ENV LD_LIBRARY_PATH="/home/appuser/tezedge/tezos/interop/lib_tezos/artifacts:/home/tezedge/tezos/interop/lib_tezos/artifacts"
CMD [ "cargo", "run", "-Zbuild-std", "--target", "x86_64-unknown-linux-gnu", "--bin", "light-node", "--", "--config-file", "/home/appuser/tezedge/deploy/tezedge.config"]
