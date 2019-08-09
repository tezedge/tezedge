#FROM ocaml/opam2:4.07
#RUN opam repo add --all-switches --set-default online https://opam.ocaml.org && \
#    opam repo set-repos online && \
#    opam update && \
#    opam install dune && \
#    sudo apt update && \
#    sudo apt install -y rsync git m4 build-essential patch unzip bubblewrap wget pkg-config libgmp-dev libev-dev libhidapi-dev

FROM debian:latest
RUN groupadd -g 999 appuser && \
    useradd -m -r -u 999 -g appuser appuser && \
    apt-get update && \
    apt-get install -y rsync git m4 build-essential patch unzip bubblewrap wget pkg-config libgmp-dev libev-dev libhidapi-dev opam
USER appuser
ENV OPAMNO 1
RUN opam init --bare --disable-sandboxing && \
    opam switch create 4.07.0 && \
    opam switch set 4.07.0 && \
    opam update && \
    opam install dune
