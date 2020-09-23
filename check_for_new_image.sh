#!/bin/bash
# ensure running bash
if ! [ -n "$BASH_VERSION" ];then
    echo "this is not bash, calling self with bash....";
    SCRIPT=$(readlink -f "$0")
    /bin/bash $SCRIPT
    exit;
fi

OWNER="adrnagy"
REPOSITORY="tezedge-private"
TAG="distroless"

LATEST="`curl https://hub.docker.com/v2/repositories/$OWNER/$REPOSITORY/tags/$TAG/?page_size=100 | jq -r '.images|.[]|.digest'`"
LATEST="$OWNER/$REPOSITORY@$LATEST"

RUNNING=`docker inspect "$OWNER/$REPOSITORY:$TAG" | jq -r '.|.[]|.RepoDigests|.[]|.'`

if [ "$RUNNING" == "$LATEST" ];then
    echo "same, do nothing"
else
    echo "update!"
    echo "$RUNNING != $LATEST"
    docker-compose down
    docker-compose pull
    docker-compose up
fi
