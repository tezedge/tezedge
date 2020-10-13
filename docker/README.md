## Building an image

Build from the repository's root directory.
Possible targets: 
- light-node: builds only the tezedge node
- sandbox: builds the node and the sandbox launcher

In the build command, we need to specify the git branch to build the node from with the help of th build argument *SOURCE_BRANCH*

```
docker build -t local:tezedge --target light-node --build-arg SOURCE_BRANCH=develop -f docker/distroless/Dockerfile .
docker build -t local:tezedge-sandbox --target sandbox --build-arg SOURCE_BRANCH=develop -f docker/distroless/Dockerfile .
```

### Push to docker hub
e.g. for sandbox develop branch:
```
docker login docker.io
docker build -t simplestakingcom/tezedge:sandbox-latest --target sandbox --build-arg SOURCE_BRANCH=develop -f distroless/Dockerfile .
docker push simplestakingcom/tezedge:sandbox-latest
```