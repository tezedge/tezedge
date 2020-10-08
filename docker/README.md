## Building an image

The first thing is to build the builder image that will contain all the necesarry tezedge binaries and libraries

```
docker build -t builder --build-arg SOURCE_BRANCH=master -f docker/builder/Dockerfile .
```

Note that we specify the git branch to build from

Next, We build a distroless image. Depending on which image you want to build (just a *light-node* or a *sandbox* version of the node) you specify the target

```
docker build --target light-node -t local:new-distroless docker/distroless
```

