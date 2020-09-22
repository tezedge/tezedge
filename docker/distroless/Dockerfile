FROM gcr.io/distroless/cc-debian10
COPY --from=builder /tmp/tezedge /

# Copy binaries
COPY --from=builder /home/appuser/tezedge/target/release/light-node /
COPY --from=builder /home/appuser/tezedge/target/release/protocol-runner /
COPY --from=builder /home/appuser/tezedge/target/release/sandbox /
COPY --from=builder /home/appuser/tezedge/sandbox/artifacts/tezos-client /
COPY --from=builder /home/appuser/tezedge/light_node/etc/tezedge_sandbox/sandbox-patch-context.json /
COPY --from=builder /home/appuser/tezedge/docker/distroless/tezedge.config /

# Copy shared libraries
COPY --from=builder /home/appuser/tezedge/tezos/interop/lib_tezos/artifacts/libtezos.so /
COPY --from=builder /usr/lib/x86_64-linux-gnu/libsodium.so.23 /usr/lib/x86_64-linux-gnu/libsodium.so.23
COPY --from=builder /usr/lib/x86_64-linux-gnu/libev.so.4 /usr/lib/x86_64-linux-gnu/libev.so.4
COPY --from=builder /usr/lib/x86_64-linux-gnu/libgmp.so.10 /usr/lib/x86_64-linux-gnu/libgmp.so.10

ENV LD_LIBRARY_PATH="/"
CMD [ "/light-node", "--config-file", "/tezedge.config"]
