#!/bin/bash
# ensure running bash
if ! [ -n "$BASH_VERSION" ];then
    echo "this is not bash, calling self with bash....";
    SCRIPT=$(readlink -f "$0")
    /bin/bash $SCRIPT
    exit;
fi

# First, run the debugger
docker-compose -f docker-compose.debugger.yml run -d --service-ports debugger

wait-for-debugger() {
    echo "Testing $1"
    until $(curl --output /dev/null --silent --fail $1); do
        printf '.'
        sleep 5
    done
    echo "OK!"
}

wait-for-debugger http://localhost:17732/v2/log

docker-compose -f docker-compose.debugger.yml run -d --service-ports rust-node
