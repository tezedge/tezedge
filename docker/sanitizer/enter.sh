#!/bin/bash

REPO_ROOT=$(readlink -f $(dirname $(readlink -f $0))/../../)
#echo $REPO_ROOT

docker build $REPO_ROOT/docker/sanitizer -t tezedge-sanitizer && docker run -v $REPO_ROOT:/home/dev/repo -it tezedge-sanitizer
