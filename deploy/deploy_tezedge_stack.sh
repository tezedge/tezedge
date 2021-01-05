#!/bin/bash
# ensure running bash
if ! [ -n "$BASH_VERSION" ];then
    echo "this is not bash, calling self with bash....";
    SCRIPT=$(readlink -f "$0")
    /bin/bash $SCRIPT
    exit;
fi

if [ -z "$1" ]; then
  echo "No tezedge root path specified. e.g.: ./deploy_tezedge_stack.sh /home/tezedge_user/tezedge v0.9.1"
  exit 1
fi

if [ -z "$2" ]; then
  echo "No tag specified. e.g.: ./deploy_tezedge_stack.sh /home/tezedge_user/tezedge v0.9.1"
  exit 1
fi

TEZEDGE_PATH="$1/deploy"

# Edit this section to set up the desired version
OWNER="simplestakingcom"    # docker hub owner
REPOSITORY="tezedge"        # docekr hub repo
TAG=$2                      # use tag "latest" for develop branch

NODE_CONTAINER_NAME="deploy_rust-node_1"
DEBUGGER_CONTAINER_NAME="deploy_debugger_1"

wait-for-debugger() {
    echo "Testing $1"
    until $(curl --output /dev/null --silent --fail $1); do
        printf '.'
        sleep 5
    done
    echo "OK!"
}

launch_stack() {
    # First, run the debugger
    docker-compose -f docker-compose.debugger.yml run -d --name $DEBUGGER_CONTAINER_NAME --service-ports debugger --user root
    # Wait for the debugger to initilize
    wait-for-debugger http://localhost:17732/v2/log
    # Then run the node
    docker-compose -f docker-compose.debugger.yml run -d --name $NODE_CONTAINER_NAME --service-ports rust-node
}

shutdown_and_update() {
    docker-compose -f docker-compose.debugger.yml down && \
    docker system prune -f -a && \
    docker volume prune -f && \
    docker-compose -f docker-compose.debugger.yml pull
}

LATEST_IMAGE_HASH="`curl https://hub.docker.com/v2/repositories/$OWNER/$REPOSITORY/tags/$TAG/?page_size=100 | jq -r '.images|.[]|.digest'`"
LATEST_IMAGE_HASH="$OWNER/$REPOSITORY@$LATEST_IMAGE_HASH"

CURRENT_IMAGE_HASH=$(docker inspect "$OWNER/$REPOSITORY:$TAG" | jq -r '.|.[]|.RepoDigests|.[]|.')
NODE_CONTAINER_STATUS=$(docker inspect $NODE_CONTAINER_NAME | jq -r '.[].State.Status')
DEBUGGER_CONTAINER_STATUS=$(docker inspect $DEBUGGER_CONTAINER_NAME | jq -r '.[].State.Status')

if [ "$NODE_CONTAINER_STATUS" == "" ];then
    echo "First run detected"
    cd $TEZEDGE_PATH && \
    launch_stack
    exit;
elif [ "$NODE_CONTAINER_STATUS" == "running" ];then
    if [ "$CURRENT_IMAGE_HASH" == "$LATEST_IMAGE_HASH" ];then
        echo "Node image unchanged"
        exit;
    else
        echo "$CURRENT_IMAGE_HASH != $LATEST_IMAGE_HASH"
        echo "Image change detected on remote. Updating image..."

        cd $TEZEDGE_PATH && \
        shutdown_and_update && \
        launch_stack
    fi
elif [ "$NODE_CONTAINER_STATUS" == "exited" ];then
    echo "Node exited"
    echo "Dumping container logs..."
    # NOTE: This will yield an empty log file with the syslog logger
    docker-compose -f docker-compose.debugger.yml logs rust-node >> node_container.log
    echo "Trying to collect node logs..."
    if [ "$DEBUGGER_CONTAINER_STATUS" == "running" ];then
        date >> node.log
        curl http://localhost:17732/v2/log?limit=500 >> node.log
    else
        echo "Cannot collect node logs, debugger not runnig"
    fi
    echo "Restarting..."
    cd $TEZEDGE_PATH && \
    shutdown_and_update && \
    launch_stack
fi
