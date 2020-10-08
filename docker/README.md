## Building an image

Build from the repository's root directory.
Possible targets: 
- light-node: builds only the tezedge node
- sandbox: builds the node and the sandbox launcher

In the build command, we need to specify the git branch to build the node from with the help of th build argument *SOURCE_BRANCH*

```
docker build -t local:tezedge --target light-node --build-arg SOURCE_BRANCH=develop -f docker/distroless/Dockerfile .
```

