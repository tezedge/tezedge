FROM debian:latest
RUN groupadd -g 999 appuser && \
    useradd -m -r -u 999 -g appuser appuser && \
    apt-get update && \
    apt-get install -y rsync git m4 build-essential patch unzip bubblewrap wget pkg-config libgmp-dev libev-dev libhidapi-dev opam curl jq
USER appuser
ENV OPAMNO 1
RUN opam init --bare --disable-sandboxing && \
    opam switch create 4.07.1 && \
    opam switch set 4.07.1 && \
    opam update && \
    opam install dune
COPY --chown=appuser:appuser . /home/appuser/build
