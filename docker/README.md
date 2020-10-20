# Building an image

## 1. Distroless
Sub-directory: `docker/distroless`

(Preferred)

Used in all our docker-compose files. Generates small images without shell.

### Build
Build from the repository's root directory.
Possible targets: 
- light-node: builds only the tezedge node
- sandbox: builds the node and the sandbox launcher

In the build command, we need to specify the git branch to build the node from with the help of th build argument *SOURCE_BRANCH*

```
docker build -t local:tezedge --target light-node --build-arg SOURCE_BRANCH=develop -f docker/distroless/Dockerfile .
docker build -t local:tezedge-sandbox --target sandbox --build-arg SOURCE_BRANCH=develop -f docker/distroless/Dockerfile .
```

### Run
No entrypoint needed.
Ports mapping:
```
    ports:
      - "4927:4927"    # WS port
      - "19732:9732"   # p2p port
      - "18732:18732"  # rpc port
```


## 2. Full
Sub-directory: `docker/full`

(Not used, probably will be removed in the future)

Generates full images with linux shell.

### Run
Need to specify entrypoint:
```
    # for light-node
    entrypoint: /home/appuser/tezedge/docker/full/tezedge.sh

    # for sandbox launcher
    entrypoint: /home/appuser/tezedge/docker/full/sandbox.sh
```

Ports mapping:
```
    ports:
      - "4927:4927"    # WS port
      - "19732:9732"   # p2p port
      - "18732:18732"  # rpc port
```

## 3. Docker for run.sh script
Sub-directory: `docker/distroless`

Used in `./run.sh docker`, when you want to build and run light-node from actual sources.

---
### Push to docker hub (example)
e.g. for sandbox develop branch:
```
docker login docker.io
docker build -t simplestakingcom/tezedge:sandbox-latest --target sandbox --build-arg SOURCE_BRANCH=develop -f distroless/Dockerfile .
docker push simplestakingcom/tezedge:sandbox-latest
```